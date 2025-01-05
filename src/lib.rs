//! A lightweight and simple workflow engine for Rust
//!
//! `tsumugi` provides a minimal yet powerful workflow engine that allows you to:
//!
//! - Define workflow steps with clear interfaces
//! - Handle data flow between steps
//! - Manage timeouts and errors
//! - Monitor execution progress
//!
//! # Example
//!
//! ```rust
//! use tsumugi::prelude::*;
//! use async_trait::async_trait;
//!
//! define_step!(SimpleStep);
//!
//! #[async_trait]
//! impl Step<String> for SimpleStep {
//!     async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<String>, WorkflowError> {
//!         ctx.insert("result", "Done!".to_string());
//!         Ok(None)
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let workflow = Workflow::builder()
//!     .add::<SimpleStep>()
//!     .start_with_type::<SimpleStep>()
//!     .build()?;
//!
//! let mut ctx = Context::new();
//! if let Err(errors) = workflow.execute(&mut ctx).await {
//!     for error in errors {
//!         println!("Error: {}", error);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

mod context;
mod error;
mod step;
mod workflow;

pub mod prelude;

pub use context::Context;
pub use error::WorkflowError;
pub use step::{Step, StepConfig};
pub use workflow::Workflow;

/// Macro to define a step with minimal boilerplate
#[macro_export]
macro_rules! define_step {
    ($name:ident) => {
        #[derive(Debug)]
        pub struct $name;

        impl Default for $name {
            fn default() -> Self {
                Self
            }
        }
    };
}
