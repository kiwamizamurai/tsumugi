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
//! - [`WorkflowError`] - Error types for workflow execution
//!
//! # Optional Traits
//!
//! - [`WithHooks`] - Add lifecycle callbacks (on_success, on_failure)
//! - [`Retryable`] - Configure retry policy
//! - [`WithTimeout`] - Configure custom timeout

mod context;
mod error;
mod step;
mod traits;

pub use context::{Context, ContextKey};
pub use error::{HookType, WorkflowError};
pub use step::{RetryPolicy, RetryPolicyError, Step, StepConfig, StepName, StepOutput};
pub use traits::{Retryable, WithHooks, WithTimeout};
