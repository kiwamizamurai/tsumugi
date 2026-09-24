//! Workflow built from closures instead of dedicated step types.
//!
//! Closure steps are handy for small pieces of logic. They can be freely
//! mixed with struct-based steps and configured with timeouts and retries.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tsumugi::prelude::*;

const SCORES: Key<Vec<u32>> = Key::new("scores");
const AVERAGE: Key<f64> = Key::new("average");
const RESULT: Key<&'static str> = Key::new("result");

/// Stand-in for a shared resource such as an HTTP client or a database pool.
#[derive(Debug, Default)]
struct ApiClient {
    calls: AtomicU32,
}

impl ApiClient {
    async fn fetch_scores(&self) -> Result<Vec<u32>, std::io::Error> {
        tokio::time::sleep(Duration::from_millis(10)).await;
        // Fail on the first call to demonstrate retries.
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(std::io::Error::other("connection reset"));
        }
        Ok(vec![72, 88, 95, 84])
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = Arc::new(ApiClient::default());

    let workflow = Workflow::builder()
        // An async closure step with a retry policy and a timeout.
        .add_async_fn("fetch", move |ctx| {
            // The closure runs once per attempt, so clone captured handles first.
            let client = Arc::clone(&client);
            Box::pin(async move {
                let scores = client.fetch_scores().await?; // any error works with `?`
                ctx.insert(SCORES, scores);
                Ok(Next::step("average"))
            })
        })
        .retry(RetryPolicy::fixed(2, Duration::from_millis(50)))
        .timeout(Duration::from_secs(5))
        // Sync closure steps for light, non-blocking logic.
        .add_fn("average", |ctx| {
            let scores = ctx.require(SCORES)?;
            let average = scores.iter().sum::<u32>() as f64 / scores.len().max(1) as f64;
            ctx.insert(AVERAGE, average);
            Ok(Next::step(if average >= 80.0 { "pass" } else { "fail" }))
        })
        .add_fn("pass", |ctx| {
            ctx.insert(RESULT, "pass");
            Ok(Next::Done)
        })
        .add_fn("fail", |ctx| {
            ctx.insert(RESULT, "fail");
            Ok(Next::Done)
        })
        .build()?;

    let mut ctx = Context::new();
    workflow.run(&mut ctx).await?;

    println!(
        "Average score: {:.1} ({})",
        ctx.require(AVERAGE)?,
        ctx.require(RESULT)?
    );
    Ok(())
}
