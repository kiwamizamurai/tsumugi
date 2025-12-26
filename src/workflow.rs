use crate::error::HookType;
use crate::step::StepName;
use crate::{Context, Step, WorkflowError};
use std::collections::HashMap;
use std::fmt;
use tokio::time::timeout;
use tracing::{info, warn};

/// A workflow engine that executes a series of steps.
///
/// `Workflow` manages a collection of steps and executes them in sequence,
/// following the transitions specified by each step's return value.
///
/// # Type Parameter
///
/// * `T` - The type of values stored in the workflow context
///
/// # Examples
///
/// ```
/// use tsumugi::prelude::*;
/// use async_trait::async_trait;
///
/// define_step!(HelloStep);
///
/// #[async_trait]
/// impl Step<String> for HelloStep {
///     async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
///         ctx.insert("greeting", "Hello, World!".to_string());
///         Ok(None)
///     }
/// }
///
/// # #[tokio::main]
/// # async fn main() {
/// let workflow = Workflow::builder()
///     .add::<HelloStep>()
///     .start_with_type::<HelloStep>()
///     .build()
///     .expect("valid workflow");
///
/// let mut ctx = Context::new();
/// workflow.execute(&mut ctx).await.expect("workflow failed");
/// assert_eq!(ctx.get("greeting"), Some(&"Hello, World!".to_string()));
/// # }
/// ```
pub struct Workflow<T> {
    steps: HashMap<StepName, Box<dyn Step<T>>>,
    start_step: StepName,
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
    /// Creates a new workflow with the given steps and start step.
    ///
    /// Prefer using [`Workflow::builder()`] for a more ergonomic API.
    pub fn new(steps: HashMap<StepName, Box<dyn Step<T>>>, start_step: StepName) -> Self {
        Self { steps, start_step }
    }

    /// Creates a new workflow builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::prelude::*;
    /// use async_trait::async_trait;
    ///
    /// define_step!(MyStep);
    ///
    /// #[async_trait]
    /// impl Step<String> for MyStep {
    ///     async fn execute(&self, _ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
    ///         Ok(None)
    ///     }
    /// }
    ///
    /// let workflow = Workflow::builder()
    ///     .add::<MyStep>()
    ///     .start_with_type::<MyStep>()
    ///     .build()
    ///     .expect("valid workflow");
    /// ```
    pub fn builder() -> WorkflowBuilder<T> {
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
    ///
    /// Steps are executed in sequence, with each step determining the next
    /// step to execute. The workflow ends when a step returns `Ok(None)`.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable reference to the context for sharing data between steps
    ///
    /// # Returns
    ///
    /// - `Ok(())` - Workflow completed successfully
    /// - `Err(Vec<WorkflowError>)` - One or more errors occurred
    ///
    /// # Errors
    ///
    /// Returns errors if:
    /// - A step fails after exhausting all retries
    /// - A step times out
    /// - A referenced step does not exist
    /// - A lifecycle hook (`on_success`, `on_failure`) fails
    ///
    /// # Examples
    ///
    /// ```
    /// # use tsumugi::prelude::*;
    /// # use async_trait::async_trait;
    /// # define_step!(MyStep);
    /// # #[async_trait]
    /// # impl Step<String> for MyStep {
    /// #     async fn execute(&self, _ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
    /// #         Ok(None)
    /// #     }
    /// # }
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let workflow = Workflow::builder()
    ///     .add::<MyStep>()
    ///     .start_with_type::<MyStep>()
    ///     .build()?;
    ///
    /// let mut ctx = Context::new();
    /// if let Err(errors) = workflow.execute(&mut ctx).await {
    ///     for error in errors {
    ///         eprintln!("Error: {}", error);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute(&self, ctx: &mut Context<T>) -> Result<(), Vec<WorkflowError>> {
        let mut current_step = Some(self.start_step.clone());
        let mut errors = Vec::new();

        while let Some(step_name) = current_step {
            let step = match self.steps.get(&step_name) {
                Some(s) => s,
                None => {
                    errors.push(WorkflowError::StepNotFound(step_name));
                    break;
                }
            };

            match self.execute_step_with_retry(step.as_ref(), ctx).await {
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

    /// Execute a single step with retry logic
    async fn execute_step_with_retry(
        &self,
        step: &dyn Step<T>,
        ctx: &mut Context<T>,
    ) -> StepResult {
        let config = step.config();
        let max_retries = config.retry_policy.max_retries();
        let timeout_duration = config.timeout.unwrap_or(std::time::Duration::from_secs(30));

        for attempt in 0..=max_retries {
            match timeout(timeout_duration, step.execute(ctx)).await {
                Ok(Ok(next_step)) => {
                    info!("Step '{}' completed successfully", step.name());
                    let hook_error = self.call_on_success(step, ctx).await;
                    return StepResult::Success(next_step).with_hook_error(hook_error);
                }
                Ok(Err(e)) => {
                    if attempt < max_retries {
                        self.log_and_wait_for_retry(step, &config, attempt, "failed")
                            .await;
                        continue;
                    }
                    warn!(
                        "Step '{}' failed after {} retries: {}",
                        step.name(),
                        attempt,
                        step.fmt_debug()
                    );
                    let hook_error = self.call_on_failure(step, ctx).await;
                    return StepResult::failed_with_errors(e, hook_error);
                }
                Err(_) => {
                    let timeout_error = WorkflowError::Timeout {
                        step_name: step.name(),
                    };
                    if attempt < max_retries {
                        self.log_and_wait_for_retry(step, &config, attempt, "timed out")
                            .await;
                        continue;
                    }
                    warn!(
                        "Step '{}' timed out after {} retries: {}",
                        step.name(),
                        attempt,
                        step.fmt_debug()
                    );
                    return StepResult::Failed(vec![timeout_error]);
                }
            }
        }

        unreachable!("Loop should always return")
    }

    async fn log_and_wait_for_retry(
        &self,
        step: &dyn Step<T>,
        config: &crate::StepConfig,
        attempt: u32,
        reason: &str,
    ) {
        let max_retries = config.retry_policy.max_retries();
        info!(
            "Step '{}' {}, retrying ({}/{})",
            step.name(),
            reason,
            attempt + 1,
            max_retries
        );
        if let Some(delay) = config.retry_policy.delay_for_attempt(attempt) {
            tokio::time::sleep(delay).await;
        }
    }

    async fn call_on_success(
        &self,
        step: &dyn Step<T>,
        ctx: &mut Context<T>,
    ) -> Option<WorkflowError> {
        if let Err(e) = step.on_success(ctx).await {
            warn!("Error in on_success handler: {}", e);
            Some(WorkflowError::HookError {
                step_name: step.name(),
                hook_type: HookType::OnSuccess,
                details: e.to_string(),
            })
        } else {
            None
        }
    }

    async fn call_on_failure(
        &self,
        step: &dyn Step<T>,
        ctx: &mut Context<T>,
    ) -> Option<WorkflowError> {
        if let Err(e) = step.on_failure(ctx).await {
            warn!("Error in on_failure handler: {}", e);
            Some(WorkflowError::HookError {
                step_name: step.name(),
                hook_type: HookType::OnFailure,
                details: e.to_string(),
            })
        } else {
            None
        }
    }
}

/// Result of executing a single step
enum StepResult {
    Success(Option<StepName>),
    Failed(Vec<WorkflowError>),
}

impl StepResult {
    fn failed_with_errors(error: WorkflowError, hook_error: Option<WorkflowError>) -> Self {
        let mut errors = Vec::new();
        if let Some(he) = hook_error {
            errors.push(he);
        }
        errors.push(error);
        StepResult::Failed(errors)
    }

    fn with_hook_error(self, hook_error: Option<WorkflowError>) -> Self {
        match (self, hook_error) {
            (StepResult::Success(_), Some(he)) => {
                // Hook error occurred after success - treat as failure
                StepResult::Failed(vec![he])
            }
            (result, None) => result,
            (StepResult::Failed(mut errs), Some(he)) => {
                errs.insert(0, he);
                StepResult::Failed(errs)
            }
        }
    }
}

/// Builder for constructing [`Workflow`] instances.
///
/// Provides a fluent API for adding steps and configuring the workflow.
///
/// # Examples
///
/// ```
/// use tsumugi::prelude::*;
/// use async_trait::async_trait;
///
/// define_step!(StepA);
/// define_step!(StepB);
///
/// #[async_trait]
/// impl Step<String> for StepA {
///     async fn execute(&self, _ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
///         Ok(Some(StepName::new("StepB")))
///     }
/// }
///
/// #[async_trait]
/// impl Step<String> for StepB {
///     async fn execute(&self, _ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
///         Ok(None)
///     }
/// }
///
/// let workflow = Workflow::builder()
///     .add::<StepA>()
///     .add::<StepB>()
///     .start_with_type::<StepA>()
///     .build()
///     .expect("valid workflow");
/// ```
pub struct WorkflowBuilder<T> {
    steps: HashMap<StepName, Box<dyn Step<T>>>,
    start_step: Option<StepName>,
}

impl<T> Default for WorkflowBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> WorkflowBuilder<T> {
    /// Creates a new empty workflow builder.
    pub fn new() -> Self {
        Self {
            steps: HashMap::new(),
            start_step: None,
        }
    }

    /// Adds a step with an explicit name.
    ///
    /// Use this when you need to specify a custom name for the step.
    ///
    /// # Arguments
    ///
    /// * `name` - The name to register the step under
    /// * `step` - The step instance
    ///
    /// # Examples
    ///
    /// ```
    /// # use tsumugi::prelude::*;
    /// # use async_trait::async_trait;
    /// # define_step!(MyStep);
    /// # #[async_trait]
    /// # impl Step<String> for MyStep {
    /// #     async fn execute(&self, _ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
    /// #         Ok(None)
    /// #     }
    /// # }
    /// let workflow = Workflow::builder()
    ///     .add_step("custom_name", MyStep)
    ///     .start_with("custom_name")
    ///     .build()
    ///     .expect("valid workflow");
    /// ```
    pub fn add_step<S: Step<T> + 'static>(mut self, name: impl Into<StepName>, step: S) -> Self {
        self.steps.insert(name.into(), Box::new(step));
        self
    }

    /// Adds a step using its type name.
    ///
    /// The step name is derived from the type's name. The step type must
    /// implement `Default`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tsumugi::prelude::*;
    /// # use async_trait::async_trait;
    /// # define_step!(MyStep);
    /// # #[async_trait]
    /// # impl Step<String> for MyStep {
    /// #     async fn execute(&self, _ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
    /// #         Ok(None)
    /// #     }
    /// # }
    /// let workflow = Workflow::builder()
    ///     .add::<MyStep>()
    ///     .start_with_type::<MyStep>()
    ///     .build()
    ///     .expect("valid workflow");
    /// ```
    pub fn add<S: Step<T> + Default + 'static>(mut self) -> Self {
        let step = S::default();
        let name = step.name();
        self.steps.insert(name, Box::new(step));
        self
    }

    /// Sets the start step by name.
    ///
    /// # Arguments
    ///
    /// * `step_name` - The name of the step to start with
    pub fn start_with(mut self, step_name: impl Into<StepName>) -> Self {
        self.start_step = Some(step_name.into());
        self
    }

    /// Sets the start step by type.
    ///
    /// The step name is derived from the type's name.
    pub fn start_with_type<S: Step<T> + 'static>(mut self) -> Self {
        self.start_step = Some(S::default_name());
        self
    }

    /// Builds the workflow.
    ///
    /// # Returns
    ///
    /// The constructed workflow if valid.
    ///
    /// # Errors
    ///
    /// Returns [`WorkflowError::Configuration`] if no start step is specified.
    /// Returns [`WorkflowError::StepNotFound`] if the start step doesn't exist.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tsumugi::prelude::*;
    /// # use async_trait::async_trait;
    /// # define_step!(MyStep);
    /// # #[async_trait]
    /// # impl Step<String> for MyStep {
    /// #     async fn execute(&self, _ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
    /// #         Ok(None)
    /// #     }
    /// # }
    /// // Missing start step
    /// let result = Workflow::<String>::builder()
    ///     .add::<MyStep>()
    ///     .build();
    /// assert!(result.is_err());
    /// ```
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
        ) -> Result<Option<StepName>, WorkflowError> {
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
        ) -> Result<Option<StepName>, WorkflowError> {
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
                assert_eq!(step_name.as_str(), "FailureStep");
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

    #[test]
    fn test_workflow_introspection() {
        let workflow = Workflow::builder()
            .add::<SuccessStep>()
            .add::<FailureStep>()
            .start_with_type::<SuccessStep>()
            .build()
            .unwrap();

        // Test start_step()
        assert_eq!(workflow.start_step().as_str(), "SuccessStep");

        // Test step_count()
        assert_eq!(workflow.step_count(), 2);

        // Test has_step()
        assert!(workflow.has_step("SuccessStep"));
        assert!(workflow.has_step("FailureStep"));
        assert!(!workflow.has_step("NonExistentStep"));

        // Test step_names()
        let names: Vec<&str> = workflow.step_names().map(|n| n.as_str()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"SuccessStep"));
        assert!(names.contains(&"FailureStep"));
    }

    use crate::step::RetryPolicy;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    // A step that fails a certain number of times before succeeding
    struct RetryableStep {
        fail_count: Arc<AtomicU32>,
        attempts: Arc<AtomicU32>,
    }

    impl std::fmt::Debug for RetryableStep {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("RetryableStep").finish()
        }
    }

    impl Default for RetryableStep {
        fn default() -> Self {
            Self {
                fail_count: Arc::new(AtomicU32::new(2)), // Fail twice, then succeed
                attempts: Arc::new(AtomicU32::new(0)),
            }
        }
    }

    #[async_trait]
    impl Step<String> for RetryableStep {
        async fn execute(
            &self,
            ctx: &mut Context<String>,
        ) -> Result<Option<StepName>, WorkflowError> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
            let fail_count = self.fail_count.load(Ordering::SeqCst);

            if attempt < fail_count {
                Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: format!("Intentional failure (attempt {})", attempt + 1),
                })
            } else {
                ctx.insert("result", "success after retries".to_string());
                Ok(None)
            }
        }

        fn config(&self) -> crate::StepConfig {
            crate::StepConfig {
                timeout: Some(std::time::Duration::from_secs(30)),
                retry_policy: RetryPolicy::fixed(3, std::time::Duration::from_millis(10)),
            }
        }
    }

    #[tokio::test]
    async fn test_workflow_retry_success() {
        let step = RetryableStep::default();
        let attempts = step.attempts.clone();

        let workflow = Workflow::builder()
            .add_step("RetryableStep", step)
            .start_with("RetryableStep")
            .build()
            .unwrap();

        let mut ctx = Context::new();
        let result = workflow.execute(&mut ctx).await;

        // Should succeed after retries
        assert!(result.is_ok());
        // Should have attempted 3 times (2 failures + 1 success)
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(
            ctx.get("result").map(|s| s.as_str()),
            Some("success after retries")
        );
    }

    #[tokio::test]
    async fn test_workflow_retry_exhausted() {
        // Create a step that always fails
        let step = RetryableStep {
            fail_count: Arc::new(AtomicU32::new(10)), // Always fail
            attempts: Arc::new(AtomicU32::new(0)),
        };
        let attempts = step.attempts.clone();

        let workflow = Workflow::builder()
            .add_step("RetryableStep", step)
            .start_with("RetryableStep")
            .build()
            .unwrap();

        let mut ctx = Context::new();
        let result = workflow.execute(&mut ctx).await;

        // Should fail after all retries exhausted
        assert!(result.is_err());
        // Should have attempted 4 times (1 initial + 3 retries)
        assert_eq!(attempts.load(Ordering::SeqCst), 4);
    }
}
