//! Core traits and types for the tsumugi workflow engine.
//!
//! This crate has no async runtime dependency. Depend on it to implement
//! reusable steps in a library; applications should use the `tsumugi` crate,
//! which re-exports everything here.
//!
//! # Overview
//!
//! - [`Step`] - A unit of work operating on a workflow state
//! - [`Next`] / [`StepResult`] - What happens after a step
//! - [`StepError`] - Errors returned by steps, convertible from any error
//! - [`RetryPolicy`] - How failed steps are retried
//! - [`Context`] / [`Key`] - A general-purpose state holding values of any type
//! - [`FnStep`] / [`AsyncFnStep`] - Steps backed by closures

mod context;
mod error;
mod fn_step;
mod step;

pub use context::{Context, Key, KeyFor, MissingValue};
pub use error::{Failure, StepError};
pub use fn_step::{AsyncFnStep, BoxFuture, FnStep};
pub use step::{Next, RetryPolicy, Step, StepName, StepResult, DEFAULT_TIMEOUT};

/// Re-export of [`async_trait`](https://docs.rs/async-trait), used to implement [`Step`].
pub use async_trait::async_trait;
