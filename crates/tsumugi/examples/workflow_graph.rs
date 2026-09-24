//! A workflow over a dedicated state type, with declared transitions, a
//! Mermaid diagram and an execution report.
//!
//! Run with `--mermaid` to print the workflow diagram instead of executing it:
//!
//! ```text
//! cargo run -p tsumugi --example workflow_graph
//! cargo run -p tsumugi --example workflow_graph -- --mermaid
//! ```

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tsumugi::prelude::*;

/// The state shared by all steps. Every field access is checked by the
/// compiler, unlike a key-value `Context`.
#[derive(Debug, Default)]
struct Order {
    total: u64,
    payment_id: Option<String>,
    rejection: Option<String>,
}

/// Simulated payment gateway that times out once before succeeding.
#[derive(Default)]
struct Charge {
    calls: AtomicU32,
}

#[async_trait]
impl Step<Order> for Charge {
    async fn run(&self, order: &mut Order) -> StepResult {
        tokio::time::sleep(Duration::from_millis(20)).await;
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err("gateway timeout".into());
        }
        order.payment_id = Some(format!("pay_{}", order.total));
        Ok(Next::step("ship"))
    }

    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::exponential(3, Duration::from_millis(50))
    }

    fn timeout(&self) -> Option<Duration> {
        Some(Duration::from_secs(5))
    }
}

fn build_workflow() -> Result<Workflow<Order>, tsumugi::BuildError> {
    WorkflowBuilder::<Order>::new()
        .add_fn("validate", |order| {
            if order.total == 0 {
                order.rejection = Some("empty order".to_string());
                return Ok(Next::step("reject"));
            }
            Ok(Next::step("charge"))
        })
        // Declared transitions are validated when the workflow is built.
        .then(["charge", "reject"])
        .add_step("charge", Charge::default())
        .then(["ship"])
        .add_fn("ship", |order| {
            println!("Shipping order paid with {:?}", order.payment_id);
            Ok(Next::Done)
        })
        .terminal()
        .add_fn("reject", |order| {
            println!("Order rejected: {:?}", order.rejection);
            Ok(Next::Done)
        })
        .terminal()
        .build()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workflow = build_workflow()?;

    if std::env::args().any(|arg| arg == "--mermaid") {
        print!("{}", workflow.to_mermaid());
        return Ok(());
    }

    let mut order = Order {
        total: 4_980,
        ..Order::default()
    };

    match workflow.run(&mut order).await {
        Ok(report) => println!("\n{}", report),
        Err(err) => {
            eprintln!("Workflow failed: {}\n\n{}", err, err.report());
            std::process::exit(1);
        }
    }

    Ok(())
}
