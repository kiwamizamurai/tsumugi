//! Declared transitions, typed keys, Mermaid diagrams and execution reports.
//!
//! Run with `--mermaid` to print the workflow diagram instead of executing it:
//!
//! ```text
//! cargo run -p tsumugi --example workflow_graph
//! cargo run -p tsumugi --example workflow_graph -- --mermaid
//! ```

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tsumugi::prelude::*;

// Typed keys: the compiler checks every read and write against the value type.
const ORDER_TOTAL: Key<u64> = Key::new("order_total");
const PAYMENT_ID: Key<String> = Key::new("payment_id");
const REJECTION: Key<String> = Key::new("rejection");

fn build_workflow() -> Result<Workflow, WorkflowError> {
    let gateway_calls = Arc::new(AtomicU32::new(0));

    // Simulated payment gateway that fails once before succeeding.
    let charge = AsyncFnStep::new("charge", move |ctx| {
        let gateway_calls = Arc::clone(&gateway_calls);
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if gateway_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(WorkflowError::StepError {
                    step_name: StepName::new("charge"),
                    details: "gateway timeout".to_string(),
                });
            }
            ctx.insert(PAYMENT_ID, "pay_42".to_string());
            Ok(StepOutput::next("ship"))
        })
    });

    Workflow::builder()
        .add_fn("validate", |ctx| {
            let total = ctx.get(ORDER_TOTAL).copied().unwrap_or_default();
            if total == 0 {
                ctx.insert(REJECTION, "empty order".to_string());
                return Ok(StepOutput::next("reject"));
            }
            Ok(StepOutput::next("charge"))
        })
        // Declared transitions are validated when the workflow is built.
        .then(["charge", "reject"])
        .add_configured(
            "charge",
            charge,
            StepConfig {
                timeout: Some(Duration::from_secs(5)),
                retry_policy: RetryPolicy::fixed(3, Duration::from_millis(50)),
            },
        )
        .then(["ship"])
        .add_fn("ship", |ctx| {
            let payment = ctx.get(PAYMENT_ID).cloned().unwrap_or_default();
            println!("Shipping order paid with {}", payment);
            Ok(StepOutput::done())
        })
        .terminal()
        .add_fn("reject", |ctx| {
            let reason = ctx.get(REJECTION).cloned().unwrap_or_default();
            println!("Order rejected: {}", reason);
            Ok(StepOutput::done())
        })
        .terminal()
        .start_with("validate")
        .build()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workflow = build_workflow()?;

    if std::env::args().any(|arg| arg == "--mermaid") {
        print!("{}", workflow.to_mermaid());
        return Ok(());
    }

    let mut ctx = Context::new();
    ctx.insert(ORDER_TOTAL, 4_980);

    match workflow.execute(&mut ctx).await {
        Ok(report) => println!("\n{}", report),
        Err(err) => {
            eprintln!("Workflow failed: {}\n\n{}", err, err.report());
            std::process::exit(1);
        }
    }

    Ok(())
}
