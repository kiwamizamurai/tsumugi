//! Workflow built from closures instead of dedicated step structs.
//!
//! Closure steps are handy for small pieces of logic. They can be freely
//! mixed with struct-based steps and configured with timeouts and retries.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tsumugi::prelude::*;

/// Stand-in for a shared resource such as an HTTP client or a database pool.
#[derive(Debug, Default)]
struct ApiClient {
    calls: AtomicU32,
}

impl ApiClient {
    async fn fetch_scores(&self) -> Result<Vec<u32>, String> {
        tokio::time::sleep(Duration::from_millis(10)).await;
        // Fail on the first call to demonstrate retries.
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err("connection reset".to_string());
        }
        Ok(vec![72, 88, 95, 84])
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = Arc::new(ApiClient::default());

    // An async closure step with a retry policy and timeout.
    let fetch = AsyncFnStep::new("fetch", move |ctx| {
        // The closure runs once per attempt, so clone captured handles first.
        let client = Arc::clone(&client);
        Box::pin(async move {
            let scores =
                client
                    .fetch_scores()
                    .await
                    .map_err(|details| WorkflowError::StepError {
                        step_name: StepName::new("fetch"),
                        details,
                    })?;
            ctx.insert("scores", scores);
            Ok(StepOutput::next("average"))
        })
    });

    let workflow = Workflow::builder()
        .add_step("fetch", fetch)
        .retry(RetryPolicy::fixed(2, Duration::from_millis(50)))
        .timeout(Duration::from_secs(5))
        // Sync closure steps for light, non-blocking logic.
        .add_fn("average", |ctx| {
            let scores = ctx.get::<Vec<u32>>("scores").cloned().unwrap_or_default();
            let average = scores.iter().sum::<u32>() as f64 / scores.len().max(1) as f64;
            ctx.insert("average", average);
            Ok(StepOutput::next(if average >= 80.0 {
                "pass"
            } else {
                "fail"
            }))
        })
        .add_fn("pass", |ctx| {
            ctx.insert("result", "pass".to_string());
            Ok(StepOutput::done())
        })
        .add_fn("fail", |ctx| {
            ctx.insert("result", "fail".to_string());
            Ok(StepOutput::done())
        })
        .start_with("fetch")
        .build()?;

    let mut ctx = Context::new();

    match workflow.execute(&mut ctx).await {
        Ok(_) => {
            let average = ctx.get::<f64>("average").copied().unwrap_or_default();
            let result = ctx.get::<String>("result").cloned().unwrap_or_default();
            println!("Average score: {:.1} ({})", average, result);
        }
        Err(err) => {
            eprintln!("Workflow failed: {}", err);
            eprintln!("{}", err.report());
        }
    }

    Ok(())
}
