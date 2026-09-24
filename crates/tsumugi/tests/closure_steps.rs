use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tsumugi::prelude::*;

#[tokio::test]
async fn test_sync_closure_step() {
    let workflow = Workflow::builder()
        .add_fn("increment", |ctx| {
            let value = ctx.get::<i32>("value").copied().unwrap_or_default();
            ctx.insert("value", value + 1);
            Ok(StepOutput::done())
        })
        .start_with("increment")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    ctx.insert("value", 41i32);
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_ok());
    assert_eq!(ctx.get::<i32>("value"), Some(&42));
}

#[tokio::test]
async fn test_async_closure_step_holds_context_across_await() {
    let workflow = Workflow::builder()
        .add_async_fn("fetch", |ctx| {
            Box::pin(async move {
                ctx.insert("status", "fetching".to_string());
                tokio::time::sleep(Duration::from_millis(1)).await;
                ctx.insert("status", "fetched".to_string());
                Ok(StepOutput::done())
            })
        })
        .start_with("fetch")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_ok());
    assert_eq!(
        ctx.get::<String>("status").map(|s| s.as_str()),
        Some("fetched")
    );
}

#[tokio::test]
async fn test_closure_branching() {
    let workflow = Workflow::builder()
        .add_fn("check", |ctx| {
            let age = ctx.get::<u32>("age").copied().unwrap_or_default();
            Ok(StepOutput::next(if age >= 18 { "adult" } else { "minor" }))
        })
        .add_fn("adult", |ctx| {
            ctx.insert("group", "adult".to_string());
            Ok(StepOutput::done())
        })
        .add_fn("minor", |ctx| {
            ctx.insert("group", "minor".to_string());
            Ok(StepOutput::done())
        })
        .start_with("check")
        .build()
        .expect("valid workflow");

    for (age, expected) in [(20u32, "adult"), (10u32, "minor")] {
        let mut ctx = Context::new();
        ctx.insert("age", age);
        let result = workflow.execute(&mut ctx).await;

        assert!(result.is_ok());
        assert_eq!(
            ctx.get::<String>("group").map(|s| s.as_str()),
            Some(expected)
        );
    }
}

#[tokio::test]
async fn test_closure_error_propagates() {
    let workflow = Workflow::builder()
        .add_fn("fail", |_ctx| {
            Err(WorkflowError::StepError {
                step_name: StepName::new("fail"),
                details: "boom".to_string(),
            })
        })
        .start_with("fail")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    let err = workflow.execute(&mut ctx).await.unwrap_err();

    assert!(matches!(
        err.error(),
        WorkflowError::StepError { step_name, details }
            if step_name.as_str() == "fail" && details == "boom"
    ));
}

#[tokio::test]
async fn test_async_closure_with_captured_state_and_retry() {
    let attempts = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&attempts);

    let step = AsyncFnStep::new("flaky", move |ctx| {
        // The closure is called once per attempt, so clone before moving.
        let counter = Arc::clone(&counter);
        Box::pin(async move {
            let attempt = counter.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt < 3 {
                return Err(WorkflowError::StepError {
                    step_name: StepName::new("flaky"),
                    details: format!("attempt {} failed", attempt),
                });
            }
            ctx.insert("attempts", attempt);
            Ok(StepOutput::done())
        })
    });

    let workflow = Workflow::builder()
        .add_step("flaky", step)
        .retry(RetryPolicy::fixed(3, Duration::from_millis(1)))
        .timeout(Duration::from_secs(1))
        .start_with("flaky")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_ok());
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    assert_eq!(ctx.get::<u32>("attempts"), Some(&3));
}

#[tokio::test]
async fn test_async_closure_timeout() {
    let step = AsyncFnStep::new("slow", |_ctx| {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(StepOutput::done())
        })
    });

    let workflow = Workflow::builder()
        .add_step("slow", step)
        .timeout(Duration::from_millis(20))
        .start_with("slow")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    let err = workflow.execute(&mut ctx).await.unwrap_err();

    assert!(matches!(
        err.error(),
        WorkflowError::Timeout { step_name } if step_name.as_str() == "slow"
    ));
}

#[derive(Debug)]
struct SaveStep;

#[async_trait]
impl Step for SaveStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        let value = ctx.get::<i32>("value").copied().unwrap_or_default();
        ctx.insert("saved", value);
        Ok(StepOutput::done())
    }

    fn name(&self) -> StepName {
        StepName::new("SaveStep")
    }
}

#[tokio::test]
async fn test_mixing_closure_and_struct_steps() {
    let workflow = Workflow::builder()
        .add_fn("prepare", |ctx| {
            ctx.insert("value", 7i32);
            Ok(StepOutput::next("save"))
        })
        .add_step("save", SaveStep)
        .start_with("prepare")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_ok());
    assert_eq!(ctx.get::<i32>("saved"), Some(&7));
}

#[tokio::test]
async fn test_closure_workflow_can_be_spawned() {
    let workflow = Arc::new(
        Workflow::builder()
            .add_async_fn("work", |ctx| {
                Box::pin(async move {
                    tokio::task::yield_now().await;
                    ctx.insert("done", true);
                    Ok(StepOutput::done())
                })
            })
            .start_with("work")
            .build()
            .expect("valid workflow"),
    );

    // Workflows with closure steps must remain usable from spawned tasks,
    // e.g. inside web framework handlers.
    let handle = tokio::spawn(async move {
        let mut ctx = Context::new();
        workflow.execute(&mut ctx).await.map(|_| ctx)
    });

    let ctx = handle
        .await
        .expect("task panicked")
        .expect("workflow failed");
    assert_eq!(ctx.get::<bool>("done"), Some(&true));
}
