use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tsumugi::prelude::*;

const EVENTS: Key<Vec<String>> = Key::new("events");

fn log(ctx: &mut Context, event: &str) {
    match ctx.get_mut(EVENTS) {
        Some(events) => events.push(event.to_string()),
        None => ctx.insert(EVENTS, vec![event.to_string()]),
    }
}

/// Step that fails `fail_times` times, records hook calls and optionally
/// fails its hooks.
#[derive(Debug, Default)]
struct HookedStep {
    fail_times: u32,
    attempts: AtomicU32,
    failing_on_success: bool,
    failing_on_failure: bool,
}

#[async_trait]
impl Step for HookedStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        log(ctx, "execute");
        if attempt < self.fail_times {
            return Err(WorkflowError::StepError {
                step_name: StepName::new("hooked"),
                details: format!("attempt {} failed", attempt + 1),
            });
        }
        Ok(StepOutput::next("after"))
    }

    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::fixed(2, Duration::from_millis(1))
    }

    async fn on_success(&self, ctx: &mut Context) -> Result<(), WorkflowError> {
        log(ctx, "on_success");
        if self.failing_on_success {
            return Err(WorkflowError::Configuration("hook broke".to_string()));
        }
        Ok(())
    }

    async fn on_failure(
        &self,
        ctx: &mut Context,
        error: &WorkflowError,
    ) -> Result<(), WorkflowError> {
        log(ctx, &format!("on_failure: {}", error));
        if self.failing_on_failure {
            return Err(WorkflowError::Configuration("hook broke".to_string()));
        }
        Ok(())
    }
}

fn workflow_with(step: HookedStep) -> Result<Workflow, WorkflowError> {
    Workflow::builder()
        .add_step("hooked", step)
        .add_fn("after", |ctx| {
            log(ctx, "after");
            Ok(StepOutput::done())
        })
        .start_with("hooked")
        .build()
}

fn events(ctx: &Context) -> Vec<&str> {
    ctx.get(EVENTS)
        .map(|events| events.iter().map(String::as_str).collect())
        .unwrap_or_default()
}

#[tokio::test]
async fn test_on_success_runs_once_after_retries_and_before_next_step() {
    let workflow = workflow_with(HookedStep {
        fail_times: 1,
        ..HookedStep::default()
    })
    .expect("valid workflow");

    let mut ctx = Context::new();
    workflow.execute(&mut ctx).await.expect("workflow succeeds");

    assert_eq!(events(&ctx), ["execute", "execute", "on_success", "after"]);
}

#[tokio::test]
async fn test_on_success_error_fails_workflow() {
    let workflow = workflow_with(HookedStep {
        failing_on_success: true,
        ..HookedStep::default()
    })
    .expect("valid workflow");

    let mut ctx = Context::new();
    let err = workflow.execute(&mut ctx).await.unwrap_err();

    assert!(matches!(
        err.error(),
        WorkflowError::HookError { step_name, hook_type: HookType::OnSuccess, .. }
            if step_name.as_str() == "hooked"
    ));
    assert_eq!(err.report().steps()[0].status(), &StepStatus::Failed);
    assert_eq!(events(&ctx), ["execute", "on_success"]);
}

#[tokio::test]
async fn test_on_failure_runs_once_after_retries_are_exhausted() {
    let workflow = workflow_with(HookedStep {
        fail_times: u32::MAX,
        ..HookedStep::default()
    })
    .expect("valid workflow");

    let mut ctx = Context::new();
    let err = workflow.execute(&mut ctx).await.unwrap_err();

    assert!(matches!(err.error(), WorkflowError::StepError { .. }));
    assert_eq!(
        events(&ctx),
        [
            "execute",
            "execute",
            "execute",
            "on_failure: Step failed: hooked, details: attempt 3 failed"
        ]
    );
}

#[tokio::test]
async fn test_on_failure_error_keeps_original_error() {
    let workflow = workflow_with(HookedStep {
        fail_times: u32::MAX,
        failing_on_failure: true,
        ..HookedStep::default()
    })
    .expect("valid workflow");

    let err = workflow.execute(&mut Context::new()).await.unwrap_err();

    assert!(matches!(err.error(), WorkflowError::StepError { .. }));
}

#[tokio::test]
async fn test_builder_retry_overrides_step_policy() {
    let step = HookedStep {
        fail_times: u32::MAX,
        ..HookedStep::default()
    };
    let workflow = Workflow::builder()
        .add_step("hooked", step)
        .retry(RetryPolicy::None)
        .start_with("hooked")
        .build()
        .expect("valid workflow");

    let err = workflow.execute(&mut Context::new()).await.unwrap_err();

    assert_eq!(err.report().steps()[0].attempts(), 1);
}

#[derive(Debug)]
struct SlowStep;

#[async_trait]
impl Step for SlowStep {
    async fn execute(&self, _ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(StepOutput::done())
    }

    fn timeout(&self) -> Option<Duration> {
        Some(Duration::from_millis(10))
    }
}

#[tokio::test]
async fn test_step_timeout_is_used_by_default() {
    let workflow = Workflow::builder()
        .add_step("slow", SlowStep)
        .start_with("slow")
        .build()
        .expect("valid workflow");

    let err = workflow.execute(&mut Context::new()).await.unwrap_err();

    assert!(matches!(err.error(), WorkflowError::Timeout { .. }));
}

#[tokio::test]
async fn test_builder_timeout_overrides_step_timeout() {
    let workflow = Workflow::builder()
        .add_step("slow", SlowStep)
        .timeout(Duration::from_secs(5))
        .start_with("slow")
        .build()
        .expect("valid workflow");

    assert!(workflow.execute(&mut Context::new()).await.is_ok());
}

#[tokio::test]
async fn test_no_timeout_disables_step_timeout() {
    let workflow = Workflow::builder()
        .add_step("slow", SlowStep)
        .no_timeout()
        .start_with("slow")
        .build()
        .expect("valid workflow");

    assert!(workflow.execute(&mut Context::new()).await.is_ok());
}

#[tokio::test]
async fn test_retry_and_timeout_combine_on_closure_steps() {
    let attempts = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&attempts);

    let workflow = Workflow::builder()
        .add_async_fn("flaky", move |_ctx| {
            let counter = Arc::clone(&counter);
            Box::pin(async move {
                // The first attempt hangs and is cut off by the timeout.
                if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
                Ok(StepOutput::done())
            })
        })
        .retry(RetryPolicy::fixed(1, Duration::from_millis(1)))
        .timeout(Duration::from_millis(20))
        .start_with("flaky")
        .build()
        .expect("valid workflow");

    let report = workflow
        .execute(&mut Context::new())
        .await
        .expect("workflow succeeds");

    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(report.total_retries(), 1);
}

#[test]
fn test_modifiers_require_a_step() {
    for builder in [
        Workflow::builder().retry(RetryPolicy::None),
        Workflow::builder().timeout(Duration::from_secs(1)),
        Workflow::builder().no_timeout(),
    ] {
        let result = builder
            .add_fn("a", |_ctx| Ok(StepOutput::done()))
            .start_with("a")
            .build();
        assert!(matches!(result, Err(WorkflowError::Configuration(_))));
    }
}
