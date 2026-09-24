//! A lightweight workflow engine for Rust.
//!
//! Workflows are built from named steps. Each step reads and writes a shared
//! [`Context`] and decides which step runs next.
//!
//! # Example
//!
//! ```
//! use tsumugi::prelude::*;
//!
//! const AMOUNT: Key<u64> = Key::new("amount");
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let workflow = Workflow::builder()
//!     .add_fn("validate", |ctx| {
//!         let amount = ctx.get(AMOUNT).copied().unwrap_or_default();
//!         Ok(StepOutput::next(if amount > 0 { "charge" } else { "reject" }))
//!     })
//!     .then(["charge", "reject"])
//!     .add_fn("charge", |_ctx| Ok(StepOutput::done()))
//!     .terminal()
//!     .add_fn("reject", |_ctx| Ok(StepOutput::done()))
//!     .terminal()
//!     .start_with("validate")
//!     .build()?;
//!
//! let mut ctx = Context::new();
//! ctx.insert(AMOUNT, 100);
//!
//! let report = workflow.execute(&mut ctx).await?;
//! println!("{}", report);
//! println!("{}", workflow.to_mermaid());
//! # Ok(())
//! # }
//! ```

mod mermaid;
mod report;
mod workflow;

// Re-export core types
pub use tsumugi_core::*;

// Export workflow types
pub use report::{ExecutionError, ExecutionReport, StepRecord, StepStatus};
pub use workflow::{Workflow, WorkflowBuilder};

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::{
        AsyncFnStep, BoxFuture, Context, ContextKey, ExecutionError, ExecutionReport, FnStep,
        HookType, Key, RetryPolicy, Retryable, Step, StepConfig, StepName, StepOutput, StepRecord,
        StepStatus, WithHooks, WithTimeout, Workflow, WorkflowBuilder, WorkflowError,
    };
}
