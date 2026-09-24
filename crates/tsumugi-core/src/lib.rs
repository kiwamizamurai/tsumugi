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
//! # Optional Traits
//!
//! - [`WithHooks`] - Add lifecycle callbacks (on_success, on_failure)
//! - [`Retryable`] - Configure retry policy
//! - [`WithTimeout`] - Configure custom timeout

mod context;
mod error;
mod fn_step;
mod step;
mod traits;

pub use context::{Context, ContextKey, Key, KeyFor};
pub use error::{HookType, WorkflowError};
pub use fn_step::{AsyncFnStep, BoxFuture, FnStep};
pub use step::{RetryPolicy, RetryPolicyError, Step, StepConfig, StepName, StepOutput};
pub use traits::{Retryable, WithHooks, WithTimeout};
