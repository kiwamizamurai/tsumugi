//! Core traits and types for tsumugi workflow engine.
//!
//! This crate provides minimal abstractions without runtime dependencies.
//! Library authors should depend on this crate to implement custom steps.
//!
//! # Core Types
//!
//! - [`Step`] - The core trait for workflow steps
//! - [`StepOutput`] - Result of step execution
//! - [`Context`] - Heterogeneous type storage for sharing data between steps
//! - [`Key`] - Context key bound to a value type
//! - [`WorkflowError`] - Error types for workflow execution
//!
//! # Closure Steps
//!
//! - [`FnStep`] - A step backed by a synchronous closure
//! - [`AsyncFnStep`] - A step backed by an asynchronous closure
//!
//! # Step Configuration
//!
//! Retry policy, timeout and lifecycle hooks are optional methods on [`Step`]
//! with sensible defaults. See the [`Step`] documentation for details.

mod context;
mod error;
mod fn_step;
mod step;

pub use context::{Context, ContextKey, Key, KeyFor};
pub use error::{HookType, WorkflowError};
pub use fn_step::{AsyncFnStep, BoxFuture, FnStep};
pub use step::{RetryPolicy, RetryPolicyError, Step, StepName, StepOutput, DEFAULT_TIMEOUT};

/// Re-export of [`async_trait`](https://docs.rs/async-trait), used to implement [`Step`].
pub use async_trait::async_trait;
