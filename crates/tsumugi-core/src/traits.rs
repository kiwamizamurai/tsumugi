//! Optional step traits for additional behavior.

use crate::context::Context;
use crate::error::WorkflowError;
use crate::step::{RetryPolicy, Step};
use async_trait::async_trait;
use std::time::Duration;

/// Optional trait for steps with lifecycle hooks.
///
/// Implement this trait to add success/failure callbacks to your step.
///
/// # Examples
///
/// ```
/// use tsumugi_core::{Step, StepOutput, StepName, Context, WorkflowError, WithHooks};
/// use async_trait::async_trait;
///
/// #[derive(Debug)]
/// struct MyStep;
///
/// #[async_trait]
/// impl Step for MyStep {
///     async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
///         Ok(StepOutput::done())
///     }
///
///     fn name(&self) -> StepName {
///         StepName::new("MyStep")
///     }
/// }
///
/// #[async_trait]
/// impl WithHooks for MyStep {
///     async fn on_success(&self, ctx: &mut Context) -> Result<(), WorkflowError> {
///         ctx.insert("success", true);
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait WithHooks: Step {
    /// Called when the step completes successfully.
    async fn on_success(&self, _ctx: &mut Context) -> Result<(), WorkflowError> {
        Ok(())
    }

    /// Called when the step fails after all retries are exhausted.
    async fn on_failure(
        &self,
        _ctx: &mut Context,
        _error: &WorkflowError,
    ) -> Result<(), WorkflowError> {
        Ok(())
    }
}

/// Optional trait for retryable steps.
///
/// Implement this trait to specify a retry policy for your step.
///
/// # Examples
///
/// ```
/// use tsumugi_core::{Step, StepOutput, StepName, Context, WorkflowError, Retryable, RetryPolicy};
/// use async_trait::async_trait;
/// use std::time::Duration;
///
/// #[derive(Debug)]
/// struct UnreliableStep;
///
/// #[async_trait]
/// impl Step for UnreliableStep {
///     async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
///         Ok(StepOutput::done())
///     }
///
///     fn name(&self) -> StepName {
///         StepName::new("UnreliableStep")
///     }
/// }
///
/// impl Retryable for UnreliableStep {
///     fn retry_policy(&self) -> RetryPolicy {
///         RetryPolicy::fixed(3, Duration::from_secs(1))
///     }
/// }
/// ```
pub trait Retryable: Step {
    /// Returns the retry policy for this step.
    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::None
    }
}

/// Optional trait for steps with custom timeout.
///
/// Implement this trait to specify a timeout for your step.
///
/// # Examples
///
/// ```
/// use tsumugi_core::{Step, StepOutput, StepName, Context, WorkflowError, WithTimeout};
/// use async_trait::async_trait;
/// use std::time::Duration;
///
/// #[derive(Debug)]
/// struct LongRunningStep;
///
/// #[async_trait]
/// impl Step for LongRunningStep {
///     async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
///         Ok(StepOutput::done())
///     }
///
///     fn name(&self) -> StepName {
///         StepName::new("LongRunningStep")
///     }
/// }
///
/// impl WithTimeout for LongRunningStep {
///     fn timeout(&self) -> Duration {
///         Duration::from_secs(120)
///     }
/// }
/// ```
pub trait WithTimeout: Step {
    /// Returns the timeout duration for this step.
    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
}
