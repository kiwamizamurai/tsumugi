//! # Tsumugi (紡)
//!
//! A lightweight and simple workflow engine for Rust.
//!
//! The name "Tsumugi" (紡) means "to spin" or "to weave" in Japanese,
//! representing how this engine elegantly weaves together different tasks
//! into a robust workflow.
//!
//! ## Features
//!
//! - **Type-safe**: [`StepName`] and [`ContextKey`] newtypes prevent typos at compile time
//! - **Async First**: Built with `async-trait` for asynchronous workflows
//! - **Retry Support**: Configurable retry policies (fixed delay, exponential backoff)
//! - **Configurable Timeouts**: Per-step timeout settings (default: 30s)
//! - **Error Handling**: Structured errors with `thiserror`, lifecycle hooks
//! - **Lightweight**: Minimal dependencies, focused on core workflow functionality
//!
//! ## Quick Start
//!
//! ```rust
//! use tsumugi::prelude::*;
//! use async_trait::async_trait;
//!
//! define_step!(LoadDataStep);
//!
//! #[async_trait]
//! impl Step<String> for LoadDataStep {
//!     async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
//!         ctx.insert("data", "Hello, Tsumugi!".to_string());
//!         Ok(None)
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let workflow = Workflow::builder()
//!     .add::<LoadDataStep>()
//!     .start_with_type::<LoadDataStep>()
//!     .build()
//!     .expect("valid workflow");
//!
//! let mut ctx = Context::new();
//! workflow.execute(&mut ctx).await.expect("workflow failed");
//!
//! assert_eq!(ctx.get("data"), Some(&"Hello, Tsumugi!".to_string()));
//! # }
//! ```
//!
//! ## Multi-Step Workflows
//!
//! ```rust
//! use tsumugi::prelude::*;
//! use async_trait::async_trait;
//!
//! define_step!(FetchStep);
//! define_step!(ProcessStep);
//!
//! #[async_trait]
//! impl Step<String> for FetchStep {
//!     async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
//!         ctx.insert("raw_data", "raw".to_string());
//!         Ok(Some(StepName::new("ProcessStep")))
//!     }
//! }
//!
//! #[async_trait]
//! impl Step<String> for ProcessStep {
//!     async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
//!         if let Some(data) = ctx.get("raw_data") {
//!             ctx.insert("processed", format!("{}_processed", data));
//!         }
//!         Ok(None)
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let workflow = Workflow::builder()
//!     .add::<FetchStep>()
//!     .add::<ProcessStep>()
//!     .start_with_type::<FetchStep>()
//!     .build()
//!     .expect("valid workflow");
//!
//! let mut ctx = Context::new();
//! workflow.execute(&mut ctx).await.expect("workflow failed");
//! # }
//! ```
//!
//! ## Retry Policies
//!
//! Configure retry behavior for unreliable operations:
//!
//! ```rust
//! use tsumugi::prelude::*;
//! use std::time::Duration;
//!
//! define_step!(UnreliableStep);
//!
//! # use async_trait::async_trait;
//! # #[async_trait]
//! # impl Step<String> for UnreliableStep {
//! #     async fn execute(&self, _ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
//! #         Ok(None)
//! #     }
//! #
//! fn config(&self) -> StepConfig {
//!     StepConfig {
//!         timeout: Some(Duration::from_secs(60)),
//!         retry_policy: RetryPolicy::exponential(5, Duration::from_millis(100)),
//!     }
//! }
//! # }
//! ```
//!
//! ## Error Handling
//!
//! ```rust
//! use tsumugi::prelude::*;
//!
//! # #[tokio::main]
//! # async fn main() {
//! # use async_trait::async_trait;
//! # define_step!(MyStep);
//! # #[async_trait]
//! # impl Step<String> for MyStep {
//! #     async fn execute(&self, _ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
//! #         Ok(None)
//! #     }
//! # }
//! # let workflow = Workflow::builder()
//! #     .add::<MyStep>()
//! #     .start_with_type::<MyStep>()
//! #     .build()
//! #     .unwrap();
//! # let mut ctx = Context::new();
//! if let Err(errors) = workflow.execute(&mut ctx).await {
//!     for error in &errors {
//!         match error {
//!             WorkflowError::StepError { step_name, details } => {
//!                 eprintln!("Step {} failed: {}", step_name, details);
//!             }
//!             WorkflowError::Timeout { step_name } => {
//!                 eprintln!("Step {} timed out", step_name);
//!             }
//!             _ => eprintln!("Error: {}", error),
//!         }
//!     }
//! }
//! # }
//! ```

mod context;
mod error;
mod step;
mod workflow;

pub mod prelude;

pub use context::{Context, ContextKey};
pub use error::{HookType, WorkflowError};
pub use step::{RetryPolicy, RetryPolicyError, Step, StepConfig, StepName};
pub use workflow::Workflow;

/// Macro to define a step with minimal boilerplate
///
/// This macro creates a step struct with:
/// - `const NAME: &'static str` - compile-time step name
/// - `Debug` derive
/// - `Default` implementation
///
/// # Example
///
/// ```rust
/// use tsumugi::define_step;
///
/// define_step!(MyStep);
/// assert_eq!(MyStep::NAME, "MyStep");
/// ```
#[macro_export]
macro_rules! define_step {
    ($name:ident) => {
        #[derive(Debug)]
        pub struct $name;

        impl $name {
            /// Step name as a compile-time constant
            #[allow(dead_code)]
            pub const NAME: &'static str = stringify!($name);
        }

        impl Default for $name {
            fn default() -> Self {
                Self
            }
        }
    };
}
