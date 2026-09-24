use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tsumugi::prelude::*;
use tsumugi::{AsyncFnStep, FnStep};

#[tokio::test]
async fn test_sync_closure_step() {
    let workflow = Workflow::builder()
        .add_fn("increment", |ctx| {
            let value = *ctx.require::<i32>("value")?;
            ctx.insert("value", value + 1);
            Ok(Next::Done)
        })
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    ctx.insert("value", 41i32);
    workflow.run(&mut ctx).await.expect("workflow succeeds");

    assert_eq!(ctx.get::<i32>("value"), Some(&42));
}

#[tokio::test]
async fn test_async_closure_holds_state_across_await() {
    let workflow = Workflow::builder()
        .add_async_fn("fetch", |ctx| {
            Box::pin(async move {
                ctx.insert("status", "fetching".to_string());
                tokio::time::sleep(Duration::from_millis(1)).await;
                ctx.insert("status", "fetched".to_string());
                Ok(Next::Done)
            })
        })
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    workflow.run(&mut ctx).await.expect("workflow succeeds");

    assert_eq!(
        ctx.get::<String>("status").map(String::as_str),
        Some("fetched")
    );
}

#[tokio::test]
async fn test_closure_branching() {
    let workflow = Workflow::builder()
        .add_fn("check", |ctx| {
            let age = *ctx.require::<u32>("age")?;
            Ok(Next::step(if age >= 18 { "adult" } else { "minor" }))
        })
        .add_fn("adult", |ctx| {
            ctx.insert("group", "adult".to_string());
            Ok(Next::Done)
        })
        .add_fn("minor", |ctx| {
            ctx.insert("group", "minor".to_string());
            Ok(Next::Done)
        })
        .build()
        .expect("valid workflow");

    for (age, expected) in [(20u32, "adult"), (10u32, "minor")] {
        let mut ctx = Context::new();
        ctx.insert("age", age);
        workflow.run(&mut ctx).await.expect("workflow succeeds");
        assert_eq!(
            ctx.get::<String>("group").map(String::as_str),
            Some(expected)
        );
    }
}

#[tokio::test]
async fn test_async_closure_with_captured_state_and_retry() {
    let attempts = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&attempts);

    let workflow = Workflow::builder()
        .add_async_fn("flaky", move |ctx| {
            // The closure is called once per attempt, so clone before moving.
            let counter = Arc::clone(&counter);
            Box::pin(async move {
                let attempt = counter.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt < 3 {
                    return Err(format!("attempt {} failed", attempt).into());
                }
                ctx.insert("attempts", attempt);
                Ok(Next::Done)
            })
        })
        .retry(RetryPolicy::fixed(3, Duration::from_millis(1)))
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    workflow.run(&mut ctx).await.expect("workflow succeeds");

    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    assert_eq!(ctx.get::<u32>("attempts"), Some(&3));
}

#[tokio::test]
async fn test_closure_step_values_can_be_registered() {
    let double = FnStep::new(|ctx: &mut Context| {
        let value = *ctx.require::<i32>("value")?;
        ctx.insert("value", value * 2);
        Ok(Next::step("announce"))
    });
    let announce = AsyncFnStep::new(|ctx: &mut Context| {
        Box::pin(async move {
            ctx.insert("announced", true);
            Ok(Next::Done)
        })
    });

    let workflow = Workflow::builder()
        .add_step("double", double)
        .add_step("announce", announce)
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    ctx.insert("value", 21i32);
    workflow.run(&mut ctx).await.expect("workflow succeeds");

    assert_eq!(ctx.get::<i32>("value"), Some(&42));
    assert_eq!(ctx.get::<bool>("announced"), Some(&true));
}

struct SaveStep;

#[async_trait]
impl Step for SaveStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        let value = *ctx.require::<i32>("value")?;
        ctx.insert("saved", value);
        Ok(Next::Done)
    }
}

#[tokio::test]
async fn test_mixing_closure_and_struct_steps() {
    let workflow = Workflow::builder()
        .add_fn("prepare", |ctx| {
            ctx.insert("value", 7i32);
            Ok(Next::step("save"))
        })
        .add_step("save", SaveStep)
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    workflow.run(&mut ctx).await.expect("workflow succeeds");

    assert_eq!(ctx.get::<i32>("saved"), Some(&7));
}
