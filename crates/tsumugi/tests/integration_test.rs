use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tsumugi::prelude::*;
use tsumugi::{ErrorKind, Failure};

struct Step1;

#[async_trait]
impl Step for Step1 {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        ctx.insert("step1", "completed".to_string());
        Ok(Next::step("step2"))
    }
}

struct Step2;

#[async_trait]
impl Step for Step2 {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        ctx.insert("step2", "completed".to_string());
        Ok(Next::Done)
    }
}

#[tokio::test]
async fn test_complete_workflow() {
    let workflow = Workflow::builder()
        .add_step("step1", Step1)
        .add_step("step2", Step2)
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    workflow.run(&mut ctx).await.expect("workflow succeeds");

    assert_eq!(
        ctx.get::<String>("step1").map(String::as_str),
        Some("completed")
    );
    assert_eq!(
        ctx.get::<String>("step2").map(String::as_str),
        Some("completed")
    );
}

#[tokio::test]
async fn test_workflow_is_reusable() {
    let workflow = Workflow::builder()
        .add_step("step1", Step1)
        .add_step("step2", Step2)
        .build()
        .expect("valid workflow");

    for _ in 0..3 {
        let mut ctx = Context::new();
        workflow.run(&mut ctx).await.expect("workflow succeeds");
        assert!(ctx.contains_key("step2"));
    }
}

#[tokio::test]
async fn test_unknown_step_error() {
    let workflow = Workflow::builder()
        .add_fn("start", |_ctx| Ok(Next::step("nonexistent")))
        .build()
        .expect("valid workflow");

    let err = workflow.run(&mut Context::new()).await.unwrap_err();

    assert_eq!(err.step(), "start");
    assert!(matches!(err.kind(), ErrorKind::UnknownStep(name) if name == "nonexistent"));
    assert_eq!(
        err.to_string(),
        "step 'start' continued to unknown step 'nonexistent'"
    );
}

struct SlowStep;

#[async_trait]
impl Step for SlowStep {
    async fn run(&self, _ctx: &mut Context) -> StepResult {
        tokio::time::sleep(Duration::from_secs(10)).await;
        Ok(Next::Done)
    }
}

#[tokio::test]
async fn test_timeout_error() {
    let workflow = Workflow::builder()
        .add_step("slow", SlowStep)
        .timeout(Duration::from_millis(20))
        .build()
        .expect("valid workflow");

    let err = workflow.run(&mut Context::new()).await.unwrap_err();

    assert_eq!(err.step(), "slow");
    assert!(err.is_timeout());
    assert!(matches!(
        err.kind(),
        ErrorKind::Step(Failure::Timeout(after)) if *after == Duration::from_millis(20)
    ));
    assert_eq!(err.to_string(), "step 'slow' timed out after 20ms");
}

struct FlakyStep {
    attempts: Arc<AtomicU32>,
    fail_until: u32,
}

#[async_trait]
impl Step for FlakyStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt < self.fail_until {
            return Err(format!("attempt {} failed", attempt + 1).into());
        }
        ctx.insert("success", true);
        Ok(Next::Done)
    }

    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::fixed(3, Duration::from_millis(1))
    }
}

#[tokio::test]
async fn test_retry_eventual_success() {
    let attempts = Arc::new(AtomicU32::new(0));
    let step = FlakyStep {
        attempts: Arc::clone(&attempts),
        fail_until: 2,
    };

    let workflow = Workflow::builder()
        .add_step("flaky", step)
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    let report = workflow.run(&mut ctx).await.expect("workflow succeeds");

    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    assert_eq!(report.total_retries(), 2);
    assert_eq!(ctx.get::<bool>("success"), Some(&true));
}

#[tokio::test]
async fn test_retry_exhausted() {
    let attempts = Arc::new(AtomicU32::new(0));
    let step = FlakyStep {
        attempts: Arc::clone(&attempts),
        fail_until: u32::MAX,
    };

    let workflow = Workflow::builder()
        .add_step("flaky", step)
        .build()
        .expect("valid workflow");

    let err = workflow.run(&mut Context::new()).await.unwrap_err();

    assert_eq!(attempts.load(Ordering::SeqCst), 4);
    assert_eq!(err.to_string(), "step 'flaky' failed: attempt 4 failed");
}

#[tokio::test]
async fn test_workflow_can_be_spawned() {
    let workflow = Arc::new(
        Workflow::builder()
            .add_async_fn("work", |ctx| {
                Box::pin(async move {
                    tokio::task::yield_now().await;
                    ctx.insert("done", true);
                    Ok(Next::Done)
                })
            })
            .build()
            .expect("valid workflow"),
    );

    // Workflows must be usable from spawned tasks, e.g. in web handlers.
    let handle = tokio::spawn(async move {
        let mut ctx = Context::new();
        workflow.run(&mut ctx).await.map(|_| ctx)
    });

    let ctx = handle
        .await
        .expect("task panicked")
        .expect("workflow failed");
    assert_eq!(ctx.get::<bool>("done"), Some(&true));
}
