//! A lightweight workflow engine for Rust.
//!
//! # Example
//!
//! ```rust,ignore
//! use tsumugi::prelude::*;
//! use async_trait::async_trait;
//!
//! #[derive(Debug)]
//! struct MyStep;
//!
//! #[async_trait]
//! impl Step for MyStep {
//!     async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
//!         ctx.insert("result", "hello".to_string());
//!         Ok(StepOutput::done())
//!     }
//!
//!     fn name(&self) -> StepName {
//!         StepName::new("MyStep")
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let workflow = Workflow::builder()
//!         .add_step("my_step", MyStep)
//!         .start_with("my_step")
//!         .build()
//!         .expect("valid workflow");
//!
//!     let mut ctx = Context::new();
//!     workflow.execute(&mut ctx).await.expect("workflow failed");
//! }
//! ```

mod workflow;

// Re-export core types
pub use tsumugi_core::*;

// Export workflow types
pub use workflow::{Workflow, WorkflowBuilder};

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::{
        Context, ContextKey, HookType, RetryPolicy, Retryable, Step, StepConfig, StepName,
        StepOutput, WithHooks, WithTimeout, Workflow, WorkflowBuilder, WorkflowError,
    };
}
