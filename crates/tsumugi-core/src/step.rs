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

/// A workflow step that can be executed asynchronously.
///
/// This is the core trait with minimal responsibilities.
/// Use optional traits like [`WithHooks`](crate::WithHooks) and
/// [`Retryable`](crate::Retryable) for additional behavior.
///
/// # Examples
///
/// ```
/// use tsumugi_core::{Step, StepOutput, StepName, Context, WorkflowError};
/// use async_trait::async_trait;
///
/// #[derive(Debug)]
/// struct ProcessDataStep;
///
/// #[async_trait]
/// impl Step for ProcessDataStep {
///     async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
///         ctx.insert("processed", true);
///         Ok(StepOutput::done())
///     }
///
///     fn name(&self) -> StepName {
///         StepName::new("ProcessData")
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

    /// Returns the step name.
    fn name(&self) -> StepName;
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
    pub fn exponential_backoff(
        max_retries: u32,
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: u32,
    ) -> Result<Self, RetryPolicyError> {
        if multiplier == 0 {
            return Err(RetryPolicyError("multiplier must be greater than 0"));
        }
        if multiplier > 10 {
            return Err(RetryPolicyError(
                "multiplier must be 10 or less to avoid overflow",
            ));
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
                let delay = initial_delay.as_millis() as u64 * (*multiplier as u64).pow(attempt);
                Some(Duration::from_millis(
                    delay.min(max_delay.as_millis() as u64),
                ))
            }
        }
    }
}

/// Configuration for a workflow step.
#[derive(Debug, Clone)]
pub struct StepConfig {
    /// Maximum time allowed for step execution.
    pub timeout: Option<Duration>,
    /// Retry policy when the step fails.
    pub retry_policy: RetryPolicy,
}

impl Default for StepConfig {
    fn default() -> Self {
        Self {
            timeout: Some(Duration::from_secs(30)),
            retry_policy: RetryPolicy::None,
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
