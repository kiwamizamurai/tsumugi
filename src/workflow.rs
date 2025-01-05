use crate::{Context, Step, WorkflowError};
use std::collections::HashMap;
use std::fmt;
use tokio::time::timeout;
use tracing::{info, warn};

pub struct Workflow<T> {
    steps: HashMap<String, Box<dyn Step<T>>>,
    start_step: String,
}

impl<T> fmt::Debug for Workflow<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workflow")
            .field("steps", &self.steps.keys().collect::<Vec<_>>())
            .field("start_step", &self.start_step)
            .finish()
    }
}

impl<T> Workflow<T> {
    pub fn new(steps: HashMap<String, Box<dyn Step<T>>>, start_step: String) -> Self {
        Self { steps, start_step }
    }

    pub fn builder() -> WorkflowBuilder<T> {
        WorkflowBuilder::new()
    }

    pub async fn execute(&self, ctx: &mut Context<T>) -> Result<(), Vec<WorkflowError>> {
        let mut current_step = Some(self.start_step.clone());
        let mut errors = Vec::new();

        while let Some(step_name) = current_step {
            let step = self.steps.get(&step_name).expect("Step not found");
            let config = step.config();

            match timeout(
                config.timeout.unwrap_or(std::time::Duration::from_secs(30)),
                step.execute(ctx),
            )
            .await
            {
                Ok(Ok(next_step)) => {
                    info!("Step '{}' completed successfully", step.name());
                    if let Err(e) = step.on_success(ctx).await {
                        warn!("Error in on_success handler: {}", e);
                    }
                    current_step = next_step;
                }
                Ok(Err(e)) => {
                    warn!("Step '{}' failed: {}", step.name(), step.fmt_debug());
                    if let Err(e) = step.on_failure(ctx).await {
                        warn!("Error in on_failure handler: {}", e);
                    }
                    errors.push(e);
                    break;
                }
                Err(_) => {
                    let timeout_error = WorkflowError::Timeout {
                        step_name: step.name(),
                    };
                    warn!("Step '{}' timed out: {}", step.name(), step.fmt_debug());
                    errors.push(timeout_error);
                    break;
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

pub struct WorkflowBuilder<T> {
    steps: HashMap<String, Box<dyn Step<T>>>,
    start_step: Option<String>,
}

impl<T> Default for WorkflowBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> WorkflowBuilder<T> {
    pub fn new() -> Self {
        Self {
            steps: HashMap::new(),
            start_step: None,
        }
    }

    /// Add a step with explicit name (legacy method)
    pub fn add_step<S: Step<T> + 'static>(mut self, name: &str, step: S) -> Self {
        self.steps.insert(name.to_string(), Box::new(step));
        self
    }

    /// Add a step using its type name
    pub fn add<S: Step<T> + Default + 'static>(mut self) -> Self {
        let step = S::default();
        let name = step.name();
        self.steps.insert(name, Box::new(step));
        self
    }

    /// Start with a step using its name (legacy method)
    pub fn start_with(mut self, step_name: &str) -> Self {
        self.start_step = Some(step_name.to_string());
        self
    }

    /// Start with a step using its type
    pub fn start_with_type<S: Step<T> + 'static>(mut self) -> Self {
        self.start_step = Some(S::default_name());
        self
    }

    pub fn build(self) -> Result<Workflow<T>, WorkflowError> {
        let start_step = self.start_step.ok_or_else(|| {
            WorkflowError::Configuration("Start step must be specified".to_string())
        })?;

        if !self.steps.contains_key(&start_step) {
            return Err(WorkflowError::StepNotFound(start_step));
        }

        Ok(Workflow::new(self.steps, start_step))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define_step;
    use async_trait::async_trait;

    define_step!(SuccessStep);

    #[async_trait]
    impl Step<String> for SuccessStep {
        async fn execute(
            &self,
            ctx: &mut Context<String>,
        ) -> Result<Option<String>, WorkflowError> {
            ctx.insert("success", "true".to_string());
            Ok(None)
        }
    }

    define_step!(FailureStep);

    #[async_trait]
    impl Step<String> for FailureStep {
        async fn execute(
            &self,
            _ctx: &mut Context<String>,
        ) -> Result<Option<String>, WorkflowError> {
            Err(WorkflowError::StepError {
                step_name: self.name(),
                details: "Intentional failure".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn test_workflow_success() {
        let workflow = Workflow::builder()
            .add::<SuccessStep>()
            .start_with_type::<SuccessStep>()
            .build()
            .unwrap();

        let mut ctx = Context::new();
        let result = workflow.execute(&mut ctx).await;
        assert!(result.is_ok());
        assert_eq!(ctx.get("success").map(|s| s.as_str()), Some("true"));
    }

    #[tokio::test]
    async fn test_workflow_failure() {
        let workflow = Workflow::builder()
            .add::<FailureStep>()
            .start_with_type::<FailureStep>()
            .build()
            .unwrap();

        let mut ctx = Context::new();
        let result = workflow.execute(&mut ctx).await;
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        match &errors[0] {
            WorkflowError::StepError { step_name, details } => {
                assert_eq!(step_name, "FailureStep");
                assert_eq!(details, "Intentional failure");
            }
            _ => panic!("Unexpected error type"),
        }
    }

    #[tokio::test]
    async fn test_workflow_builder_validation() {
        let result = Workflow::<String>::builder().add::<SuccessStep>().build();
        assert!(result.is_err());
        match result.unwrap_err() {
            WorkflowError::Configuration(msg) => {
                assert_eq!(msg, "Start step must be specified");
            }
            _ => panic!("Unexpected error type"),
        }
    }
}
