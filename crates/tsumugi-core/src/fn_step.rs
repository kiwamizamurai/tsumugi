//! Closure-based steps.
//!
//! [`FnStep`] and [`AsyncFnStep`] let you define a step from a closure instead of
//! a dedicated struct with a manual [`Step`] implementation. Both implement
//! [`Step`], so they can be registered with any `WorkflowBuilder` method,
//! including ones that configure timeouts and retry policies.

use crate::context::Context;
use crate::error::WorkflowError;
use crate::step::{Step, StepName, StepOutput};
use async_trait::async_trait;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

/// A boxed, `Send` future borrowing data for `'a`.
///
/// This is the return type of closures passed to [`AsyncFnStep`]. Wrap an
/// `async move` block with `Box::pin` to produce one.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A step backed by a synchronous closure.
///
/// Use this for quick, CPU-light logic such as transformations, validation or
/// branching. The closure runs on the async executor thread, so it must not
/// block; a blocking closure also cannot be interrupted by the step timeout.
/// Use [`AsyncFnStep`] for anything that performs I/O.
///
/// The closure may be called more than once when a retry policy is configured.
///
/// # Examples
///
/// ```
/// use tsumugi_core::{Context, FnStep, Step, StepOutput};
///
/// let step = FnStep::new("double", |ctx: &mut Context| {
///     let value = ctx.get::<i32>("value").copied().unwrap_or_default();
///     ctx.insert("value", value * 2);
///     Ok(StepOutput::done())
/// });
///
/// assert_eq!(step.name().as_str(), "double");
/// ```
pub struct FnStep<F> {
    name: StepName,
    func: F,
}

impl<F> FnStep<F>
where
    F: Fn(&mut Context) -> Result<StepOutput, WorkflowError> + Send + Sync,
{
    /// Creates a new step from a synchronous closure.
    pub fn new(name: impl Into<StepName>, func: F) -> Self {
        Self {
            name: name.into(),
            func,
        }
    }
}

impl<F> fmt::Debug for FnStep<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FnStep")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl<F> Step for FnStep<F>
where
    F: Fn(&mut Context) -> Result<StepOutput, WorkflowError> + Send + Sync,
{
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        (self.func)(ctx)
    }

    fn name(&self) -> StepName {
        self.name.clone()
    }
}

/// A step backed by an asynchronous closure.
///
/// The closure receives the context and returns a [`BoxFuture`] that may borrow
/// it, so the step can read and write the context across `.await` points:
///
/// ```ignore
/// |ctx| Box::pin(async move { /* use ctx */ Ok(StepOutput::done()) })
/// ```
///
/// The closure may be called more than once when a retry policy is configured,
/// so clone any captured values (such as an `Arc` client) inside it before
/// moving them into the `async` block.
///
/// # Examples
///
/// ```
/// use tsumugi_core::{AsyncFnStep, Context, Step, StepOutput};
///
/// let step = AsyncFnStep::new("greet", |ctx: &mut Context| {
///     Box::pin(async move {
///         ctx.insert("greeting", "hello".to_string());
///         Ok(StepOutput::done())
///     })
/// });
///
/// assert_eq!(step.name().as_str(), "greet");
/// ```
pub struct AsyncFnStep<F> {
    name: StepName,
    func: F,
}

impl<F> AsyncFnStep<F>
where
    F: for<'a> Fn(&'a mut Context) -> BoxFuture<'a, Result<StepOutput, WorkflowError>>
        + Send
        + Sync,
{
    /// Creates a new step from an asynchronous closure.
    pub fn new(name: impl Into<StepName>, func: F) -> Self {
        Self {
            name: name.into(),
            func,
        }
    }
}

impl<F> fmt::Debug for AsyncFnStep<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncFnStep")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl<F> Step for AsyncFnStep<F>
where
    F: for<'a> Fn(&'a mut Context) -> BoxFuture<'a, Result<StepOutput, WorkflowError>>
        + Send
        + Sync,
{
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        (self.func)(ctx).await
    }

    fn name(&self) -> StepName {
        self.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fn_step_debug() {
        let step = FnStep::new("sync", |_ctx: &mut Context| Ok(StepOutput::done()));
        assert_eq!(
            format!("{:?}", step),
            "FnStep { name: StepName(\"sync\"), .. }"
        );
    }

    #[test]
    fn test_async_fn_step_debug() {
        let step = AsyncFnStep::new("async", |_ctx: &mut Context| {
            Box::pin(async move { Ok(StepOutput::done()) })
        });
        assert_eq!(
            format!("{:?}", step),
            "AsyncFnStep { name: StepName(\"async\"), .. }"
        );
    }
}
