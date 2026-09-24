//! Closure-based steps.
//!
//! [`FnStep`] and [`AsyncFnStep`] turn closures into [`Step`]s. Workflow
//! builders usually create them for you (`add_fn` and `add_async_fn`); use
//! these types directly to store or pass around closure steps.

use crate::step::{Step, StepResult};
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
/// use tsumugi_core::{Context, FnStep, Next};
///
/// let step = FnStep::new(|ctx: &mut Context| {
///     let value = ctx.require::<i32>("value")?;
///     ctx.insert("doubled", value * 2);
///     Ok(Next::Done)
/// });
/// ```
pub struct FnStep<F>(F);

impl<F> FnStep<F> {
    /// Creates a step from a synchronous closure.
    pub fn new<S>(func: F) -> Self
    where
        F: Fn(&mut S) -> StepResult,
    {
        Self(func)
    }
}

impl<F> fmt::Debug for FnStep<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FnStep(..)")
    }
}

#[async_trait]
impl<S, F> Step<S> for FnStep<F>
where
    S: Send,
    F: Fn(&mut S) -> StepResult + Send + Sync,
{
    async fn run(&self, state: &mut S) -> StepResult {
        (self.0)(state)
    }
}

/// A step backed by an asynchronous closure.
///
/// The closure receives the state and returns a [`BoxFuture`] that may borrow
/// it, so the step can read and write the state across `.await` points:
///
/// ```text
/// |ctx| Box::pin(async move { /* use ctx */ Ok(Next::Done) })
/// ```
///
/// The closure may be called more than once when a retry policy is configured,
/// so clone any captured values (such as an `Arc` client) inside it before
/// moving them into the `async` block.
///
/// # Examples
///
/// ```
/// use tsumugi_core::{AsyncFnStep, Context, Next};
///
/// let step = AsyncFnStep::new(|ctx: &mut Context| {
///     Box::pin(async move {
///         ctx.insert("greeting", "hello".to_string());
///         Ok(Next::Done)
///     })
/// });
/// ```
pub struct AsyncFnStep<F>(F);

impl<F> AsyncFnStep<F> {
    /// Creates a step from an asynchronous closure.
    pub fn new<S>(func: F) -> Self
    where
        F: for<'a> Fn(&'a mut S) -> BoxFuture<'a, StepResult>,
    {
        Self(func)
    }
}

impl<F> fmt::Debug for AsyncFnStep<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AsyncFnStep(..)")
    }
}

#[async_trait]
impl<S, F> Step<S> for AsyncFnStep<F>
where
    S: Send,
    F: for<'a> Fn(&'a mut S) -> BoxFuture<'a, StepResult> + Send + Sync,
{
    async fn run(&self, state: &mut S) -> StepResult {
        (self.0)(state).await
    }
}
