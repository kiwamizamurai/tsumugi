//! Workflow engine for executing steps.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn};
use tsumugi_core::{
    Context, Retryable, Step, StepConfig, StepName, StepOutput, WithTimeout, WorkflowError,
};

/// A workflow engine that executes a series of steps.
pub struct Workflow {
    steps: HashMap<StepName, StepEntry>,
    start_step: StepName,
}

struct StepEntry {
    step: Box<dyn Step>,
    timeout: Duration,
    retry_policy: tsumugi_core::RetryPolicy,
}

impl fmt::Debug for Workflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workflow")
            .field("steps", &self.steps.keys().collect::<Vec<_>>())
            .field("start_step", &self.start_step)
            .finish()
    }
}

impl Workflow {
    /// Creates a new workflow builder.
    pub fn builder() -> WorkflowBuilder {
        WorkflowBuilder::new()
    }

    /// Returns the name of the start step.
    pub fn start_step(&self) -> &StepName {
        &self.start_step
    }

    /// Returns an iterator over all registered step names.
    pub fn step_names(&self) -> impl Iterator<Item = &StepName> {
        self.steps.keys()
    }

    /// Returns `true` if a step with the given name exists.
    pub fn has_step(&self, name: &str) -> bool {
        self.steps.contains_key(name)
    }

    /// Returns the number of registered steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Executes the workflow starting from the configured start step.
    pub async fn execute(&self, ctx: &mut Context) -> Result<(), Vec<WorkflowError>> {
        let mut current_step = Some(self.start_step.clone());
        let mut errors = Vec::new();

        while let Some(step_name) = current_step {
            let entry = match self.steps.get(&step_name) {
                Some(s) => s,
                None => {
                    errors.push(WorkflowError::StepNotFound(step_name));
                    break;
                }
            };

            match self.execute_step_with_retry(entry, ctx).await {
                StepResult::Success(next) => {
                    current_step = next;
                }
                StepResult::Failed(step_errors) => {
                    errors.extend(step_errors);
                    current_step = None;
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    async fn execute_step_with_retry(&self, entry: &StepEntry, ctx: &mut Context) -> StepResult {
        let max_retries = entry.retry_policy.max_retries();
        let timeout_duration = entry.timeout;

        for attempt in 0..=max_retries {
            match timeout(timeout_duration, entry.step.execute(ctx)).await {
                Ok(Ok(output)) => {
                    info!("Step '{}' completed successfully", entry.step.name());
                    let next = match output {
                        StepOutput::Continue(name) => Some(name),
                        StepOutput::Complete => None,
                    };
                    // Note: hook calling would require trait object casting which is complex
                    // For now, we skip hooks for basic Step implementations
                    return StepResult::Success(next);
                }
                Ok(Err(e)) => {
                    if attempt < max_retries {
                        self.log_and_wait_for_retry(entry, attempt, "failed").await;
                        continue;
                    }
                    warn!(
                        "Step '{}' failed after {} retries",
                        entry.step.name(),
                        attempt
                    );
                    return StepResult::Failed(vec![e]);
                }
                Err(_) => {
                    let timeout_error = WorkflowError::Timeout {
                        step_name: entry.step.name(),
                    };
                    if attempt < max_retries {
                        self.log_and_wait_for_retry(entry, attempt, "timed out")
                            .await;
                        continue;
                    }
                    warn!(
                        "Step '{}' timed out after {} retries",
                        entry.step.name(),
                        attempt
                    );
                    return StepResult::Failed(vec![timeout_error]);
                }
            }
        }

        unreachable!("Loop should always return")
    }

    async fn log_and_wait_for_retry(&self, entry: &StepEntry, attempt: u32, reason: &str) {
        let max_retries = entry.retry_policy.max_retries();
        info!(
            "Step '{}' {}, retrying ({}/{})",
            entry.step.name(),
            reason,
            attempt + 1,
            max_retries
        );
        if let Some(delay) = entry.retry_policy.delay_for_attempt(attempt) {
            tokio::time::sleep(delay).await;
        }
    }
}

enum StepResult {
    Success(Option<StepName>),
    Failed(Vec<WorkflowError>),
}

/// Builder for constructing [`Workflow`] instances.
#[derive(Default)]
pub struct WorkflowBuilder {
    steps: HashMap<StepName, StepEntry>,
    start_step: Option<StepName>,
}

impl WorkflowBuilder {
    /// Creates a new empty workflow builder.
    pub fn new() -> Self {
        Self {
            steps: HashMap::new(),
            start_step: None,
        }
    }

    /// Adds a step with an explicit name.
    pub fn add_step<S: Step + 'static>(mut self, name: impl Into<StepName>, step: S) -> Self {
        let step_name = name.into();
        self.steps.insert(
            step_name,
            StepEntry {
                step: Box::new(step),
                timeout: Duration::from_secs(30),
                retry_policy: tsumugi_core::RetryPolicy::None,
            },
        );
        self
    }

    /// Adds a retryable step with an explicit name.
    pub fn add_retryable<S: Retryable + 'static>(
        mut self,
        name: impl Into<StepName>,
        step: S,
    ) -> Self {
        let step_name = name.into();
        let retry_policy = step.retry_policy();
        self.steps.insert(
            step_name,
            StepEntry {
                step: Box::new(step),
                timeout: Duration::from_secs(30),
                retry_policy,
            },
        );
        self
    }

    /// Adds a step with custom timeout.
    pub fn add_with_timeout<S: Step + 'static>(
        mut self,
        name: impl Into<StepName>,
        step: S,
        timeout: Duration,
    ) -> Self {
        let step_name = name.into();
        self.steps.insert(
            step_name,
            StepEntry {
                step: Box::new(step),
                timeout,
                retry_policy: tsumugi_core::RetryPolicy::None,
            },
        );
        self
    }

    /// Adds a step that implements WithTimeout trait.
    pub fn add_with_timeout_trait<S: WithTimeout + 'static>(
        mut self,
        name: impl Into<StepName>,
        step: S,
    ) -> Self {
        let step_name = name.into();
        let timeout = step.timeout();
        self.steps.insert(
            step_name,
            StepEntry {
                step: Box::new(step),
                timeout,
                retry_policy: tsumugi_core::RetryPolicy::None,
            },
        );
        self
    }

    /// Adds a fully configured step.
    pub fn add_configured<S: Step + 'static>(
        mut self,
        name: impl Into<StepName>,
        step: S,
        config: StepConfig,
    ) -> Self {
        let step_name = name.into();
        self.steps.insert(
            step_name,
            StepEntry {
                step: Box::new(step),
                timeout: config.timeout.unwrap_or(Duration::from_secs(30)),
                retry_policy: config.retry_policy,
            },
        );
        self
    }

    /// Sets the start step by name.
    pub fn start_with(mut self, step_name: impl Into<StepName>) -> Self {
        self.start_step = Some(step_name.into());
        self
    }

    /// Builds the workflow.
    pub fn build(self) -> Result<Workflow, WorkflowError> {
        let start_step = self.start_step.ok_or_else(|| {
            WorkflowError::Configuration("Start step must be specified".to_string())
        })?;

        if !self.steps.contains_key(&start_step) {
            return Err(WorkflowError::StepNotFound(start_step));
        }

        Ok(Workflow {
            steps: self.steps,
            start_step,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    #[derive(Debug)]
    struct SuccessStep;

    #[async_trait]
    impl Step for SuccessStep {
        async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
            ctx.insert("success", true);
            Ok(StepOutput::done())
        }

        fn name(&self) -> StepName {
            StepName::new("SuccessStep")
        }
    }

    #[derive(Debug)]
    struct FailureStep;

    #[async_trait]
    impl Step for FailureStep {
        async fn execute(&self, _ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
            Err(WorkflowError::StepError {
                step_name: self.name(),
                details: "Intentional failure".to_string(),
            })
        }

        fn name(&self) -> StepName {
            StepName::new("FailureStep")
        }
    }

    #[tokio::test]
    async fn test_workflow_success() {
        let workflow = Workflow::builder()
            .add_step("success", SuccessStep)
            .start_with("success")
            .build()
            .expect("valid workflow");

        let mut ctx = Context::new();
        let result = workflow.execute(&mut ctx).await;
        assert!(result.is_ok());
        assert_eq!(ctx.get::<bool>("success"), Some(&true));
    }

    #[tokio::test]
    async fn test_workflow_failure() {
        let workflow = Workflow::builder()
            .add_step("failure", FailureStep)
            .start_with("failure")
            .build()
            .expect("valid workflow");

        let mut ctx = Context::new();
        let result = workflow.execute(&mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_workflow_builder_validation() {
        let result = Workflow::builder().add_step("step", SuccessStep).build();
        assert!(result.is_err());
    }
}
