use crate::context::Context;
use crate::error::WorkflowError;
use async_trait::async_trait;
use std::fmt;
use std::time::Duration;

/// Type-safe step name wrapper.
///
/// Provides compile-time safety for step identifiers, preventing
/// typos and mismatched step names at the API level.
///
/// # Examples
///
/// ```
/// use tsumugi::StepName;
///
/// let name = StepName::new("ProcessData");
/// assert_eq!(name.as_str(), "ProcessData");
///
/// // From trait for ergonomic conversion
/// let name: StepName = "ValidateInput".into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StepName(String);

impl StepName {
    /// Creates a new StepName
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Creates a StepName from a type's name (extracts last segment)
    pub fn from_type_name<T: ?Sized>() -> Self {
        let full_name = std::any::type_name::<T>();
        let short_name = full_name.split("::").last().unwrap_or("UnknownStep");
        Self::new(short_name)
    }

    /// Returns the step name as a string slice
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

/// Retry policy for step execution.
///
/// Defines how a step should be retried when it fails. Supports no retry,
/// fixed delay, and exponential backoff strategies.
///
/// # Examples
///
/// ```
/// use tsumugi::RetryPolicy;
/// use std::time::Duration;
///
/// // No retry (default)
/// let policy = RetryPolicy::None;
///
/// // Fixed delay: retry 3 times with 1 second delay
/// let policy = RetryPolicy::fixed(3, Duration::from_secs(1));
///
/// // Exponential backoff: retry 5 times starting at 100ms
/// let policy = RetryPolicy::exponential(5, Duration::from_millis(100));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RetryPolicy {
    /// No retry - fail immediately on error.
    #[default]
    None,
    /// Fixed delay between retries.
    Fixed {
        /// Maximum number of retry attempts
        max_retries: u32,
        /// Delay between each retry
        delay: Duration,
    },
    /// Exponential backoff with configurable parameters.
    ExponentialBackoff {
        /// Maximum number of retry attempts
        max_retries: u32,
        /// Initial delay before first retry
        initial_delay: Duration,
        /// Maximum delay cap
        max_delay: Duration,
        /// Multiplier for each retry (e.g., 2 doubles the delay)
        multiplier: u32,
    },
}

/// Error returned when [`RetryPolicy`] configuration is invalid.
///
/// This error is returned by [`RetryPolicy::exponential_backoff`] when
/// the provided parameters are invalid.
///
/// # Examples
///
/// ```
/// use tsumugi::{RetryPolicy, RetryPolicyError};
/// use std::time::Duration;
///
/// // Invalid: multiplier is 0
/// let result = RetryPolicy::exponential_backoff(
///     3,
///     Duration::from_millis(100),
///     Duration::from_secs(10),
///     0,
/// );
/// assert!(result.is_err());
/// ```
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
    ///
    /// Retries the step up to `max_retries` times with a constant `delay`
    /// between each attempt.
    ///
    /// # Arguments
    ///
    /// * `max_retries` - Maximum number of retry attempts
    /// * `delay` - Fixed delay between retries
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::RetryPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::fixed(3, Duration::from_secs(1));
    /// assert_eq!(policy.max_retries(), 3);
    /// assert_eq!(policy.delay_for_attempt(0), Some(Duration::from_secs(1)));
    /// assert_eq!(policy.delay_for_attempt(2), Some(Duration::from_secs(1)));
    /// ```
    pub fn fixed(max_retries: u32, delay: Duration) -> Self {
        RetryPolicy::Fixed { max_retries, delay }
    }

    /// Creates an exponential backoff retry policy with default settings.
    ///
    /// Uses `multiplier=2` and `max_delay=60s`. The delay doubles after
    /// each attempt until reaching the maximum.
    ///
    /// # Arguments
    ///
    /// * `max_retries` - Maximum number of retry attempts
    /// * `initial_delay` - Delay before the first retry
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::RetryPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::exponential(5, Duration::from_millis(100));
    ///
    /// // Delays: 100ms, 200ms, 400ms, 800ms, 1600ms
    /// assert_eq!(policy.delay_for_attempt(0), Some(Duration::from_millis(100)));
    /// assert_eq!(policy.delay_for_attempt(1), Some(Duration::from_millis(200)));
    /// assert_eq!(policy.delay_for_attempt(2), Some(Duration::from_millis(400)));
    /// ```
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
    /// # Arguments
    ///
    /// * `max_retries` - Maximum number of retry attempts
    /// * `initial_delay` - Delay before the first retry
    /// * `max_delay` - Maximum delay cap (delays won't exceed this)
    /// * `multiplier` - Factor to multiply delay by after each attempt (1-10)
    ///
    /// # Errors
    ///
    /// Returns [`RetryPolicyError`] if:
    /// - `multiplier` is 0 (would result in no backoff)
    /// - `multiplier` is greater than 10 (risk of overflow)
    /// - `max_delay` is less than `initial_delay`
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::RetryPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::exponential_backoff(
    ///     5,
    ///     Duration::from_millis(100),
    ///     Duration::from_secs(30),
    ///     3,  // Triple the delay each time
    /// )?;
    ///
    /// // Delays: 100ms, 300ms, 900ms, 2700ms, 8100ms
    /// # Ok::<(), tsumugi::RetryPolicyError>(())
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::RetryPolicy;
    /// use std::time::Duration;
    ///
    /// assert_eq!(RetryPolicy::None.max_retries(), 0);
    /// assert_eq!(RetryPolicy::fixed(5, Duration::from_secs(1)).max_retries(), 5);
    /// ```
    pub fn max_retries(&self) -> u32 {
        match self {
            RetryPolicy::None => 0,
            RetryPolicy::Fixed { max_retries, .. } => *max_retries,
            RetryPolicy::ExponentialBackoff { max_retries, .. } => *max_retries,
        }
    }

    /// Calculates the delay for the given retry attempt.
    ///
    /// Attempt numbers are 0-indexed (first retry is attempt 0).
    ///
    /// # Returns
    ///
    /// - `None` for `RetryPolicy::None`
    /// - `Some(delay)` for other policies
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::RetryPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::exponential(3, Duration::from_millis(100));
    ///
    /// assert_eq!(policy.delay_for_attempt(0), Some(Duration::from_millis(100)));
    /// assert_eq!(policy.delay_for_attempt(1), Some(Duration::from_millis(200)));
    /// ```
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
///
/// Controls timeout and retry behavior for step execution.
///
/// # Examples
///
/// ```
/// use tsumugi::{StepConfig, RetryPolicy};
/// use std::time::Duration;
///
/// let config = StepConfig {
///     timeout: Some(Duration::from_secs(60)),
///     retry_policy: RetryPolicy::fixed(3, Duration::from_secs(1)),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct StepConfig {
    /// Maximum time allowed for step execution. `None` means no timeout.
    /// Default: 30 seconds.
    pub timeout: Option<Duration>,
    /// Retry policy when the step fails. Default: no retry.
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

/// A workflow step that can be executed asynchronously.
///
/// Implement this trait to define custom steps in your workflow. Each step
/// receives a mutable context for data sharing and returns the name of the
/// next step to execute (or `None` to end the workflow).
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
/// define_step!(ProcessDataStep);
///
/// #[async_trait]
/// impl Step<String> for ProcessDataStep {
///     async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
///         // Process data and store result
///         ctx.insert("processed", "done".to_string());
///
///         // Return next step or None to end workflow
///         Ok(None)
///     }
///
///     fn config(&self) -> StepConfig {
///         StepConfig {
///             timeout: Some(std::time::Duration::from_secs(60)),
///             retry_policy: RetryPolicy::fixed(3, std::time::Duration::from_secs(1)),
///         }
///     }
/// }
/// ```
#[async_trait]
pub trait Step<T>: Send + Sync {
    /// Executes the step logic.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable reference to the workflow context for reading/writing data
    ///
    /// # Returns
    ///
    /// - `Ok(Some(step_name))` - Continue to the specified step
    /// - `Ok(None)` - End the workflow successfully
    /// - `Err(error)` - Step failed (may trigger retry based on config)
    async fn execute(&self, ctx: &mut Context<T>) -> Result<Option<StepName>, WorkflowError>;

    /// Returns the step name.
    ///
    /// By default, uses the type name. Override to provide a custom name.
    fn name(&self) -> StepName {
        StepName::from_type_name::<Self>()
    }

    /// Returns the default step name from the type.
    ///
    /// Used by the builder when referencing steps by type.
    fn default_name() -> StepName
    where
        Self: Sized,
    {
        StepName::from_type_name::<Self>()
    }

    /// Returns the step configuration.
    ///
    /// Override to customize timeout and retry behavior.
    fn config(&self) -> StepConfig {
        StepConfig::default()
    }

    /// Called when the step fails after all retries are exhausted.
    ///
    /// Use for cleanup or error reporting. Default implementation does nothing.
    async fn on_failure(&self, _ctx: &mut Context<T>) -> Result<(), WorkflowError> {
        Ok(())
    }

    /// Called when the step completes successfully.
    ///
    /// Use for logging or post-processing. Default implementation does nothing.
    async fn on_success(&self, _ctx: &mut Context<T>) -> Result<(), WorkflowError> {
        Ok(())
    }

    /// Formats step information for debugging.
    fn fmt_debug(&self) -> String {
        format!("Step '{}' (config: {:?})", self.name(), self.config())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define_step;
    use async_trait::async_trait;

    define_step!(TestStep);

    #[async_trait]
    impl Step<String> for TestStep {
        async fn execute(
            &self,
            ctx: &mut Context<String>,
        ) -> Result<Option<StepName>, WorkflowError> {
            ctx.insert("test", "executed".to_string());
            Ok(Some(StepName::new("next_step")))
        }
    }

    #[tokio::test]
    async fn test_step_execution() {
        let step = TestStep;
        let mut ctx = Context::new();

        let result = step.execute(&mut ctx).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(StepName::new("next_step")));
        assert_eq!(ctx.get("test").map(|s| s.as_str()), Some("executed"));
    }

    #[test]
    fn test_step_name() {
        let step = TestStep;
        assert_eq!(step.name(), StepName::new("TestStep"));
    }

    #[test]
    fn test_step_config() {
        let step = TestStep;
        let config = step.config();
        assert_eq!(config.timeout, Some(Duration::from_secs(30)));
        assert_eq!(config.retry_policy, RetryPolicy::None);
    }

    #[test]
    fn test_retry_policy_none() {
        let policy = RetryPolicy::None;
        assert_eq!(policy.max_retries(), 0);
        assert_eq!(policy.delay_for_attempt(0), None);
    }

    #[test]
    fn test_retry_policy_fixed() {
        let policy = RetryPolicy::fixed(3, Duration::from_secs(1));
        assert_eq!(policy.max_retries(), 3);
        assert_eq!(policy.delay_for_attempt(0), Some(Duration::from_secs(1)));
        assert_eq!(policy.delay_for_attempt(2), Some(Duration::from_secs(1)));
    }

    #[test]
    fn test_retry_policy_exponential() {
        let policy = RetryPolicy::ExponentialBackoff {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2,
        };
        assert_eq!(policy.max_retries(), 5);
        // attempt 0: 100ms * 2^0 = 100ms
        assert_eq!(
            policy.delay_for_attempt(0),
            Some(Duration::from_millis(100))
        );
        // attempt 1: 100ms * 2^1 = 200ms
        assert_eq!(
            policy.delay_for_attempt(1),
            Some(Duration::from_millis(200))
        );
        // attempt 2: 100ms * 2^2 = 400ms
        assert_eq!(
            policy.delay_for_attempt(2),
            Some(Duration::from_millis(400))
        );
        // attempt 10: should be capped at max_delay (10s)
        assert_eq!(policy.delay_for_attempt(10), Some(Duration::from_secs(10)));
    }

    #[test]
    fn test_retry_policy_exponential_backoff_validation() {
        // Valid configuration
        let result = RetryPolicy::exponential_backoff(
            3,
            Duration::from_millis(100),
            Duration::from_secs(10),
            2,
        );
        assert!(result.is_ok());

        // multiplier = 0 is invalid
        let result = RetryPolicy::exponential_backoff(
            3,
            Duration::from_millis(100),
            Duration::from_secs(10),
            0,
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, "multiplier must be greater than 0");

        // multiplier > 10 is invalid (overflow risk)
        let result = RetryPolicy::exponential_backoff(
            3,
            Duration::from_millis(100),
            Duration::from_secs(10),
            11,
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().0,
            "multiplier must be 10 or less to avoid overflow"
        );

        // max_delay < initial_delay is invalid
        let result = RetryPolicy::exponential_backoff(
            3,
            Duration::from_secs(10),
            Duration::from_millis(100),
            2,
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, "max_delay must be >= initial_delay");
    }
}
