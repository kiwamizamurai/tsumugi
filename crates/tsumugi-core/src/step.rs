//! The step trait and related types.

use crate::context::Context;
use crate::error::{Failure, StepError};
use async_trait::async_trait;
use std::borrow::Borrow;
use std::fmt;
use std::time::Duration;

/// The name a step is registered under.
///
/// Step names identify steps within a workflow: they are the targets of
/// [`Next::step`] and appear in logs, errors and reports.
///
/// Any string converts into a `StepName`. To get compile-time checked step
/// names, implement `From<YourEnum> for StepName`:
///
/// ```
/// use tsumugi_core::{Next, StepName};
///
/// enum Order { Charge, Ship }
///
/// impl From<Order> for StepName {
///     fn from(step: Order) -> Self {
///         StepName::new(match step {
///             Order::Charge => "charge",
///             Order::Ship => "ship",
///         })
///     }
/// }
///
/// assert_eq!(Next::step(Order::Ship), Next::step("ship"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StepName(String);

impl StepName {
    /// Creates a new step name.
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
        f.write_str(&self.0)
    }
}

impl From<&str> for StepName {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl From<String> for StepName {
    fn from(name: String) -> Self {
        Self(name)
    }
}

impl From<&StepName> for StepName {
    fn from(name: &StepName) -> Self {
        name.clone()
    }
}

impl AsRef<str> for StepName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for StepName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for StepName {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for StepName {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// What a workflow does after a step succeeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// Run the named step.
    Step(StepName),
    /// Complete the workflow.
    Done,
}

impl Next {
    /// Continues with the named step.
    pub fn step(name: impl Into<StepName>) -> Self {
        Next::Step(name.into())
    }
}

/// The result of running a step.
pub type StepResult = Result<Next, StepError>;

/// Timeout applied to a step unless it, or its registration, specifies
/// otherwise.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// A unit of work in a workflow, operating on a state of type `S`.
///
/// The state defaults to [`Context`], a map that can hold values of any type.
/// For workflows whose steps are known up front, a dedicated state struct
/// lets the compiler check every field access:
///
/// ```
/// use tsumugi_core::{async_trait, Next, Step, StepResult};
///
/// #[derive(Default)]
/// struct Order {
///     total: u64,
///     approved: bool,
/// }
///
/// struct Approve;
///
/// #[async_trait]
/// impl Step<Order> for Approve {
///     async fn run(&self, order: &mut Order) -> StepResult {
///         order.approved = order.total < 10_000;
///         Ok(Next::Done)
///     }
/// }
/// ```
///
/// A step can also be generic over any state that provides what it needs,
/// which makes it reusable across workflows:
///
/// ```
/// use tsumugi_core::{async_trait, Next, Step, StepResult};
///
/// trait HasLog: Send {
///     fn log(&mut self) -> &mut Vec<String>;
/// }
///
/// struct Audit(&'static str);
///
/// #[async_trait]
/// impl<S: HasLog> Step<S> for Audit {
///     async fn run(&self, state: &mut S) -> StepResult {
///         state.log().push(self.0.to_string());
///         Ok(Next::Done)
///     }
/// }
/// ```
///
/// Only [`run`](Step::run) is required. The other methods configure the step
/// and have defaults:
///
/// | Method | Default |
/// |--------|---------|
/// | [`retry_policy`](Step::retry_policy) | [`RetryPolicy::none`] |
/// | [`timeout`](Step::timeout) | [`DEFAULT_TIMEOUT`] (30 seconds) |
/// | [`on_success`](Step::on_success) | does nothing |
/// | [`on_failure`](Step::on_failure) | does nothing |
///
/// Retry policy and timeout can also be set when registering the step, which
/// takes precedence over these methods.
#[async_trait]
pub trait Step<S = Context>: Send + Sync
where
    S: Send,
{
    /// Runs the step and decides what happens next.
    async fn run(&self, state: &mut S) -> StepResult;

    /// Returns the retry policy applied when [`run`](Step::run) fails or
    /// times out.
    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::none()
    }

    /// Returns the maximum duration of a single attempt, or `None` for no
    /// timeout.
    fn timeout(&self) -> Option<Duration> {
        Some(DEFAULT_TIMEOUT)
    }

    /// Called once after the step succeeds, before the next step runs.
    ///
    /// Returning an error fails the workflow. The hook is not subject to the
    /// step timeout and is not retried.
    async fn on_success(&self, _state: &mut S) -> Result<(), StepError> {
        Ok(())
    }

    /// Called once when the step has failed and all retries are exhausted,
    /// e.g. for cleanup or compensation. The workflow then fails with the
    /// original error.
    async fn on_failure(&self, _state: &mut S, _failure: &Failure) {}
}

/// How often and how quickly a failed step is retried.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use tsumugi_core::RetryPolicy;
///
/// // Up to 3 retries, 1 second apart.
/// let fixed = RetryPolicy::fixed(3, Duration::from_secs(1));
///
/// // Up to 5 retries after 100ms, 200ms, 400ms, 800ms and 1s.
/// let exponential = RetryPolicy::exponential(5, Duration::from_millis(100))
///     .max_delay(Duration::from_secs(1));
///
/// assert_eq!(exponential.delay(3), Duration::from_millis(800));
/// assert_eq!(exponential.delay(4), Duration::from_secs(1));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    max_retries: u32,
    initial_delay: Duration,
    max_delay: Duration,
    factor: f64,
}

impl RetryPolicy {
    /// Never retries.
    pub const fn none() -> Self {
        Self {
            max_retries: 0,
            initial_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
            factor: 1.0,
        }
    }

    /// Retries up to `max_retries` times, waiting `delay` before each retry.
    pub const fn fixed(max_retries: u32, delay: Duration) -> Self {
        Self {
            max_retries,
            initial_delay: delay,
            max_delay: delay,
            factor: 1.0,
        }
    }

    /// Retries up to `max_retries` times, doubling the delay after each retry,
    /// starting at `initial_delay` and capped at 60 seconds.
    ///
    /// Use [`factor`](Self::factor) and [`max_delay`](Self::max_delay) to
    /// adjust the growth.
    pub const fn exponential(max_retries: u32, initial_delay: Duration) -> Self {
        Self {
            max_retries,
            initial_delay,
            max_delay: Duration::from_secs(60),
            factor: 2.0,
        }
    }

    /// Sets the growth factor of the delay. Values below 1.0 (including NaN)
    /// are treated as 1.0.
    #[must_use]
    pub fn factor(mut self, factor: f64) -> Self {
        self.factor = if factor >= 1.0 { factor } else { 1.0 };
        self
    }

    /// Caps the delay between retries.
    #[must_use]
    pub fn max_delay(mut self, max_delay: Duration) -> Self {
        self.max_delay = max_delay;
        self
    }

    /// Returns the maximum number of retries.
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    /// Returns the delay before retry number `retry` (starting at 0).
    pub fn delay(&self, retry: u32) -> Duration {
        let exponent = i32::try_from(retry).unwrap_or(i32::MAX);
        let seconds = self.initial_delay.as_secs_f64() * self.factor.powi(exponent);
        // Saturate at the cap instead of overflowing for large retry numbers.
        Duration::try_from_secs_f64(seconds)
            .unwrap_or(self.max_delay)
            .min(self.max_delay)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_name() {
        let name = StepName::new("test");
        assert_eq!(name.as_str(), "test");
        assert_eq!(name, "test");
        assert_eq!(StepName::from("test"), name);
    }

    #[test]
    fn test_next() {
        assert_eq!(Next::step("next"), Next::Step(StepName::new("next")));
    }

    #[test]
    fn test_retry_policy_none() {
        let policy = RetryPolicy::none();
        assert_eq!(policy.max_retries(), 0);
        assert_eq!(policy, RetryPolicy::default());
    }

    #[test]
    fn test_retry_policy_fixed() {
        let policy = RetryPolicy::fixed(3, Duration::from_secs(1));
        assert_eq!(policy.max_retries(), 3);
        assert_eq!(policy.delay(0), Duration::from_secs(1));
        assert_eq!(policy.delay(10), Duration::from_secs(1));
    }

    #[test]
    fn test_retry_policy_exponential() {
        let policy = RetryPolicy::exponential(5, Duration::from_millis(100));
        assert_eq!(policy.max_retries(), 5);
        assert_eq!(policy.delay(0), Duration::from_millis(100));
        assert_eq!(policy.delay(1), Duration::from_millis(200));
        assert_eq!(policy.delay(2), Duration::from_millis(400));
    }

    #[test]
    fn test_retry_policy_custom_factor() {
        let policy = RetryPolicy::exponential(5, Duration::from_millis(100)).factor(1.5);
        assert_eq!(policy.delay(2), Duration::from_millis(225));

        let clamped = RetryPolicy::exponential(5, Duration::from_millis(100)).factor(0.5);
        assert_eq!(clamped.delay(3), Duration::from_millis(100));

        let nan = RetryPolicy::exponential(5, Duration::from_millis(100)).factor(f64::NAN);
        assert_eq!(nan.delay(3), Duration::from_millis(100));
    }

    #[test]
    fn test_retry_policy_saturates() {
        let policy = RetryPolicy::exponential(u32::MAX, Duration::from_millis(100))
            .factor(10.0)
            .max_delay(Duration::from_secs(5));

        assert_eq!(policy.delay(1), Duration::from_secs(1));
        assert_eq!(policy.delay(2), Duration::from_secs(5));
        assert_eq!(policy.delay(1_000), Duration::from_secs(5));
        assert_eq!(policy.delay(u32::MAX), Duration::from_secs(5));
    }

    #[test]
    fn test_retry_policy_keeps_sub_millisecond_precision() {
        let policy = RetryPolicy::exponential(3, Duration::from_micros(500));
        assert_eq!(policy.delay(1), Duration::from_micros(1000));
    }
}
