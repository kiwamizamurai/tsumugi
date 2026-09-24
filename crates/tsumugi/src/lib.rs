//! A lightweight, embeddable workflow engine for Rust.
//!
//! A workflow is a set of named [`Step`]s operating on a shared state. Each
//! step decides which step runs [`Next`]; tsumugi takes care of retries,
//! timeouts, hooks, validation and reporting. No database, queue or server is
//! involved: workflows run inside your process.
//!
//! # Example
//!
//! ```
//! use std::time::Duration;
//! use tsumugi::prelude::*;
//!
//! // The state shared by all steps. Any `Send` type works; `Context` is a
//! // ready-made map for loosely coupled steps.
//! #[derive(Default)]
//! struct Order {
//!     total: u64,
//!     payment_id: Option<String>,
//! }
//!
//! // Steps are closures or types implementing `Step`.
//! struct Charge;
//!
//! #[async_trait]
//! impl Step<Order> for Charge {
//!     async fn run(&self, order: &mut Order) -> StepResult {
//!         if order.total == 0 {
//!             return Err("nothing to charge".into());
//!         }
//!         order.payment_id = Some(format!("pay_{}", order.total));
//!         Ok(Next::step("ship"))
//!     }
//!
//!     fn retry_policy(&self) -> RetryPolicy {
//!         RetryPolicy::exponential(3, Duration::from_millis(100))
//!     }
//! }
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let workflow = WorkflowBuilder::<Order>::new()
//!     .add_step("charge", Charge)
//!     .then(["ship"])
//!     .add_fn("ship", |order| {
//!         println!("shipping order paid with {:?}", order.payment_id);
//!         Ok(Next::Done)
//!     })
//!     .terminal()
//!     .build()?; // validates names and transitions
//!
//! let mut order = Order { total: 4_980, ..Default::default() };
//! let report = workflow.run(&mut order).await?;
//! println!("{}", report);
//! println!("{}", workflow.to_mermaid());
//! # Ok(())
//! # }
//! ```

/// Compiles the code examples in the README.
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
struct ReadmeDoctests;

mod error;
mod mermaid;
mod report;
mod workflow;

// Re-export core types
pub use tsumugi_core::*;

// Export workflow types
pub use error::{BuildError, ErrorKind, ExecutionError};
pub use report::{ExecutionReport, StepRecord, StepStatus};
pub use workflow::{Workflow, WorkflowBuilder};

/// Prelude for convenient imports.
///
/// Includes the [`async_trait`] attribute needed to implement [`Step`].
pub mod prelude {
    pub use crate::{
        async_trait, BuildError, Context, ExecutionError, Failure, Key, Next, RetryPolicy, Step,
        StepError, StepName, StepResult, Workflow, WorkflowBuilder,
    };
}
