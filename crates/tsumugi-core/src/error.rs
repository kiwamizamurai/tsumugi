//! Errors produced by steps.

use std::error::Error;
use std::fmt;
use std::time::Duration;

/// An error returned by a step.
///
/// `StepError` wraps any error type, so steps can use `?` on any `Result`
/// whose error implements [`std::error::Error`], as well as on strings:
///
/// ```
/// use tsumugi_core::{Context, Next, StepError, StepResult};
///
/// fn parse(ctx: &mut Context) -> StepResult {
///     let raw = ctx.require::<String>("raw")?;      // MissingValue
///     let value: u32 = raw.parse()?;                // ParseIntError
///     if value == 0 {
///         return Err("value must not be zero".into()); // &str
///     }
///     ctx.insert("value", value);
///     Ok(Next::Done)
/// }
///
/// let mut ctx = Context::new();
/// ctx.insert("raw", "abc".to_string());
/// let error: StepError = parse(&mut ctx).unwrap_err();
/// assert!(error.downcast_ref::<std::num::ParseIntError>().is_some());
/// ```
///
/// The workflow engine records which step failed, so the error does not
/// need to repeat it.
///
/// Like `anyhow::Error`, `StepError` does not itself implement
/// [`std::error::Error`] (that would conflict with the blanket conversion);
/// use [`as_error`](Self::as_error) or [`into_inner`](Self::into_inner) to
/// access the underlying error.
pub struct StepError {
    inner: Box<dyn Error + Send + Sync + 'static>,
}

impl StepError {
    /// Creates a step error from an error or a message.
    pub fn new(error: impl Into<Box<dyn Error + Send + Sync + 'static>>) -> Self {
        Self {
            inner: error.into(),
        }
    }

    /// Returns a reference to the underlying error.
    pub fn as_error(&self) -> &(dyn Error + Send + Sync + 'static) {
        &*self.inner
    }

    /// Returns the underlying error.
    pub fn into_inner(self) -> Box<dyn Error + Send + Sync + 'static> {
        self.inner
    }

    /// Returns the underlying error if it is of type `E`.
    pub fn downcast_ref<E: Error + 'static>(&self) -> Option<&E> {
        self.inner.downcast_ref::<E>()
    }
}

impl<E> From<E> for StepError
where
    E: Into<Box<dyn Error + Send + Sync + 'static>>,
{
    fn from(error: E) -> Self {
        Self::new(error)
    }
}

impl fmt::Debug for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl AsRef<dyn Error + Send + Sync + 'static> for StepError {
    fn as_ref(&self) -> &(dyn Error + Send + Sync + 'static) {
        self.as_error()
    }
}

/// How a step failed once its retries were exhausted.
///
/// Passed to [`Step::on_failure`](crate::Step::on_failure).
#[derive(Debug)]
#[non_exhaustive]
pub enum Failure {
    /// The last attempt returned an error.
    Error(StepError),
    /// The last attempt exceeded the step timeout.
    Timeout(Duration),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Error(error) => fmt::Display::fmt(error, f),
            Failure::Timeout(after) => write!(f, "timed out after {:?}", after),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Custom;

    impl fmt::Display for Custom {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "custom")
        }
    }

    impl Error for Custom {}

    #[test]
    fn test_conversions() {
        let from_str: StepError = "message".into();
        let from_string: StepError = String::from("owned").into();
        let from_error: StepError = Custom.into();

        assert_eq!(from_str.to_string(), "message");
        assert_eq!(from_string.to_string(), "owned");
        assert!(from_error.downcast_ref::<Custom>().is_some());
        assert_eq!(from_error.as_error().to_string(), "custom");
    }

    #[test]
    fn test_failure_display() {
        let error = Failure::Error("boom".into());
        let timeout = Failure::Timeout(Duration::from_millis(1500));

        assert_eq!(error.to_string(), "boom");
        assert_eq!(timeout.to_string(), "timed out after 1.5s");
    }
}
