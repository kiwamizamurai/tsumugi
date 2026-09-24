//! Step trait and related types.

use crate::context::Context;
use crate::error::WorkflowError;
use async_trait::async_trait;
use std::fmt::{self, Debug};
use std::time::Duration;

/// Type-safe step name wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StepName(String);

impl StepName {
    /// Creates a new StepName.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the step name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StepName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for StepName {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for StepName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for StepName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for StepName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// Output from a step execution.
///
/// Represents what should happen after a step completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutput {
    /// Continue to the specified step.
    Continue(StepName),
    /// Workflow completed successfully.
    Complete,
}

impl StepOutput {
    /// Creates a Continue output to the next step.
    pub fn next(name: impl Into<StepName>) -> Self {
        Self::Continue(name.into())
    }

    /// Creates a Complete output.
    pub fn done() -> Self {
        Self::Complete
    }
}

/// Timeout applied to a step unless it, or its registration, specifies otherwise.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// A workflow step that can be executed asynchronously.
///
/// Only [`execute`](Step::execute) is required. The other methods have default
/// implementations that can be overridden to configure the step:
///
/// | Method | Default |
/// |--------|---------|
/// | [`retry_policy`](Step::retry_policy) | [`RetryPolicy::None`] |
/// | [`timeout`](Step::timeout) | [`DEFAULT_TIMEOUT`] (30 seconds) |
/// | [`on_success`](Step::on_success) | does nothing |
/// | [`on_failure`](Step::on_failure) | does nothing |
///
/// Retry policy and timeout can also be set when registering the step, which
/// takes precedence over these methods.
///
/// # Examples
///
/// ```
/// use tsumugi_core::{async_trait, Context, RetryPolicy, Step, StepOutput, WorkflowError};
/// use std::time::Duration;
///
/// #[derive(Debug)]
/// struct FetchStep;
///
/// #[async_trait]
/// impl Step for FetchStep {
///     async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
///         ctx.insert("fetched", true);
///         Ok(StepOutput::next("save"))
///     }
///
///     fn retry_policy(&self) -> RetryPolicy {
///         RetryPolicy::fixed(3, Duration::from_millis(100))
///     }
///
///     fn timeout(&self) -> Option<Duration> {
///         Some(Duration::from_secs(10))
///     }
///
///     async fn on_failure(&self, ctx: &mut Context, error: &WorkflowError) -> Result<(), WorkflowError> {
///         ctx.insert("fetch_error", error.to_string());
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait Step: Send + Sync + Debug {
    /// Executes the step logic.
    ///
    /// # Returns
    ///
    /// - `Ok(StepOutput::Continue(name))` - Continue to the specified step
    /// - `Ok(StepOutput::Complete)` - End the workflow successfully
    /// - `Err(error)` - Step failed
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError>;

    /// Returns a descriptive name for this step implementation.
    ///
    /// Workflows identify steps by the name they are registered under, which
    /// is what appears in logs, errors and execution reports. This name is only
    /// a label for the implementation itself and defaults to its type name.
    fn name(&self) -> StepName {
        StepName::new(std::any::type_name::<Self>())
    }

    /// Returns the retry policy applied when [`execute`](Step::execute) fails
    /// or times out.
    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::None
    }

    /// Returns the maximum duration of a single attempt, or `None` for no
    /// timeout.
    fn timeout(&self) -> Option<Duration> {
        Some(DEFAULT_TIMEOUT)
    }

    /// Called once after the step succeeds, before moving to the next step.
    ///
    /// Returning an error fails the workflow with
    /// [`WorkflowError::HookError`]. The hook is not subject to the step
    /// timeout and is not retried.
    async fn on_success(&self, _ctx: &mut Context) -> Result<(), WorkflowError> {
        Ok(())
    }

    /// Called once when the step has failed and all retries are exhausted.
    ///
    /// Use it for cleanup or compensation. The workflow still fails with the
    /// original error; an error returned by this hook is only logged.
    async fn on_failure(
        &self,
        _ctx: &mut Context,
        _error: &WorkflowError,
    ) -> Result<(), WorkflowError> {
        Ok(())
    }
}

/// Retry policy for step execution.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RetryPolicy {
    /// No retry - fail immediately on error.
    #[default]
    None,
    /// Fixed delay between retries.
    Fixed {
        /// Maximum number of retry attempts.
        max_retries: u32,
        /// Delay between each retry.
        delay: Duration,
    },
    /// Exponential backoff with configurable parameters.
    ExponentialBackoff {
        /// Maximum number of retry attempts.
        max_retries: u32,
        /// Initial delay before first retry.
        initial_delay: Duration,
        /// Maximum delay cap.
        max_delay: Duration,
        /// Multiplier for each retry.
        multiplier: u32,
    },
}

/// Error returned when [`RetryPolicy`] configuration is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicyError(pub &'static str);

impl std::fmt::Display for RetryPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for RetryPolicyError {}

impl RetryPolicy {
    /// Creates a fixed retry policy.
    pub fn fixed(max_retries: u32, delay: Duration) -> Self {
        RetryPolicy::Fixed { max_retries, delay }
    }

    /// Creates an exponential backoff retry policy with default settings.
    pub fn exponential(max_retries: u32, initial_delay: Duration) -> Self {
        RetryPolicy::ExponentialBackoff {
            max_retries,
            initial_delay,
            max_delay: Duration::from_secs(60),
            multiplier: 2,
        }
    }

    /// Creates an exponential backoff retry policy with custom settings.
    ///
    /// The delay before retry `n` (starting at 0) is
    /// `initial_delay * multiplier^n`, capped at `max_delay`.
    ///
    /// # Errors
    ///
    /// Returns an error if `multiplier` is 0 or `max_delay < initial_delay`.
    pub fn exponential_backoff(
        max_retries: u32,
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: u32,
    ) -> Result<Self, RetryPolicyError> {
        if multiplier == 0 {
            return Err(RetryPolicyError("multiplier must be greater than 0"));
        }
        if max_delay < initial_delay {
            return Err(RetryPolicyError("max_delay must be >= initial_delay"));
        }
        Ok(RetryPolicy::ExponentialBackoff {
            max_retries,
            initial_delay,
            max_delay,
            multiplier,
        })
    }

    /// Returns the maximum number of retries for this policy.
    pub fn max_retries(&self) -> u32 {
        match self {
            RetryPolicy::None => 0,
            RetryPolicy::Fixed { max_retries, .. } => *max_retries,
            RetryPolicy::ExponentialBackoff { max_retries, .. } => *max_retries,
        }
    }

    /// Calculates the delay for the given retry attempt.
    pub fn delay_for_attempt(&self, attempt: u32) -> Option<Duration> {
        match self {
            RetryPolicy::None => None,
            RetryPolicy::Fixed { delay, .. } => Some(*delay),
            RetryPolicy::ExponentialBackoff {
                initial_delay,
                max_delay,
                multiplier,
                ..
            } => {
                // Saturate instead of overflowing for large attempt numbers.
                let delay = multiplier
                    .checked_pow(attempt)
                    .and_then(|factor| initial_delay.checked_mul(factor))
                    .unwrap_or(*max_delay);
                Some(delay.min(*max_delay))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_name() {
        let name = StepName::new("test");
        assert_eq!(name.as_str(), "test");

        let name: StepName = "test".into();
        assert_eq!(name.as_str(), "test");
    }

    #[test]
    fn test_step_output() {
        let output = StepOutput::next("next_step");
        assert_eq!(output, StepOutput::Continue(StepName::new("next_step")));

        let output = StepOutput::done();
        assert_eq!(output, StepOutput::Complete);
    }

    #[test]
    fn test_retry_policy_fixed() {
        let policy = RetryPolicy::fixed(3, Duration::from_secs(1));
        assert_eq!(policy.max_retries(), 3);
        assert_eq!(policy.delay_for_attempt(0), Some(Duration::from_secs(1)));
    }

    #[test]
    fn test_retry_policy_exponential() {
        let policy = RetryPolicy::exponential(5, Duration::from_millis(100));
        assert_eq!(policy.max_retries(), 5);
        assert_eq!(
            policy.delay_for_attempt(0),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            policy.delay_for_attempt(1),
            Some(Duration::from_millis(200))
        );
    }

    #[test]
    fn test_exponential_backoff_saturates() {
        let policy = RetryPolicy::exponential_backoff(
            u32::MAX,
            Duration::from_millis(100),
            Duration::from_secs(5),
            10,
        )
        .expect("valid policy");

        assert_eq!(policy.delay_for_attempt(1), Some(Duration::from_secs(1)));
        assert_eq!(policy.delay_for_attempt(2), Some(Duration::from_secs(5)));
        assert_eq!(policy.delay_for_attempt(64), Some(Duration::from_secs(5)));
        assert_eq!(
            policy.delay_for_attempt(u32::MAX),
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    fn test_exponential_backoff_keeps_sub_millisecond_precision() {
        let policy = RetryPolicy::exponential(3, Duration::from_micros(500));
        assert_eq!(
            policy.delay_for_attempt(1),
            Some(Duration::from_micros(1000))
        );
    }

    #[test]
    fn test_retry_policy_validation() {
        let result = RetryPolicy::exponential_backoff(
            3,
            Duration::from_millis(100),
            Duration::from_secs(10),
            0,
        );
        assert!(result.is_err());
    }
}
