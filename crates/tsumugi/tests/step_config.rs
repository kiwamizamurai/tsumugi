//! Retry policies, timeouts and lifecycle hooks.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tsumugi::prelude::*;
use tsumugi::{BuildError, ErrorKind, Failure, StepStatus};

#[derive(Default)]
struct State {
    events: Vec<String>,
}

impl State {
    fn log(&mut self, event: impl Into<String>) {
        self.events.push(event.into());
    }
}

/// Step that fails `fail_times` times and records hook calls.
#[derive(Default)]
struct HookedStep {
    fail_times: u32,
    attempts: AtomicU32,
    failing_on_success: bool,
}

#[async_trait]
impl Step<State> for HookedStep {
    async fn run(&self, state: &mut State) -> StepResult {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        state.log("run");
        if attempt < self.fail_times {
            return Err(format!("attempt {} failed", attempt + 1).into());
        }
        Ok(Next::step("after"))
    }

    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::fixed(2, Duration::from_millis(1))
    }

    async fn on_success(&self, state: &mut State) -> Result<(), StepError> {
        state.log("on_success");
        if self.failing_on_success {
            return Err("hook broke".into());
        }
        Ok(())
    }

    async fn on_failure(&self, state: &mut State, failure: &Failure) {
        state.log(format!("on_failure: {}", failure));
    }
}

fn workflow_with(step: HookedStep) -> Result<Workflow<State>, BuildError> {
    WorkflowBuilder::<State>::new()
        .add_step("hooked", step)
        .add_fn("after", |state| {
            state.log("after");
            Ok(Next::Done)
        })
        .build()
}

#[tokio::test]
async fn test_on_success_runs_once_after_retries_and_before_next_step() {
    let workflow = workflow_with(HookedStep {
        fail_times: 1,
        ..HookedStep::default()
    })
    .expect("valid workflow");

    let mut state = State::default();
    workflow.run(&mut state).await.expect("workflow succeeds");

    assert_eq!(state.events, ["run", "run", "on_success", "after"]);
}

#[tokio::test]
async fn test_on_success_error_fails_workflow() {
    let workflow = workflow_with(HookedStep {
        failing_on_success: true,
        ..HookedStep::default()
    })
    .expect("valid workflow");

    let mut state = State::default();
    let err = workflow.run(&mut state).await.unwrap_err();

    assert_eq!(err.step(), "hooked");
    assert!(matches!(err.kind(), ErrorKind::Hook(e) if e.to_string() == "hook broke"));
    assert_eq!(
        err.to_string(),
        "on_success hook of step 'hooked' failed: hook broke"
    );
    assert_eq!(err.report().steps()[0].status(), &StepStatus::Failed);
    assert_eq!(state.events, ["run", "on_success"]);
}

#[tokio::test]
async fn test_on_failure_runs_once_after_retries_are_exhausted() {
    let workflow = workflow_with(HookedStep {
        fail_times: u32::MAX,
        ..HookedStep::default()
    })
    .expect("valid workflow");

    let mut state = State::default();
    let err = workflow.run(&mut state).await.unwrap_err();

    assert!(matches!(err.kind(), ErrorKind::Step(Failure::Error(_))));
    assert_eq!(
        state.events,
        ["run", "run", "run", "on_failure: attempt 3 failed"]
    );
}

#[tokio::test]
async fn test_builder_retry_overrides_step_policy() {
    let workflow = WorkflowBuilder::<State>::new()
        .add_step(
            "hooked",
            HookedStep {
                fail_times: u32::MAX,
                ..HookedStep::default()
            },
        )
        .retry(RetryPolicy::none())
        .build()
        .expect("valid workflow");

    let err = workflow.run(&mut State::default()).await.unwrap_err();

    assert_eq!(err.report().steps()[0].attempts(), 1);
}

struct SlowStep;

#[async_trait]
impl Step for SlowStep {
    async fn run(&self, _ctx: &mut Context) -> StepResult {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(Next::Done)
    }

    fn timeout(&self) -> Option<Duration> {
        Some(Duration::from_millis(10))
    }
}

#[tokio::test]
async fn test_step_timeout_is_used_by_default() {
    let workflow = Workflow::builder()
        .add_step("slow", SlowStep)
        .build()
        .expect("valid workflow");

    let err = workflow.run(&mut Context::new()).await.unwrap_err();

    assert!(err.is_timeout());
}

#[tokio::test]
async fn test_builder_timeout_overrides_step_timeout() {
    let workflow = Workflow::builder()
        .add_step("slow", SlowStep)
        .timeout(Duration::from_secs(5))
        .build()
        .expect("valid workflow");

    assert!(workflow.run(&mut Context::new()).await.is_ok());
}

#[tokio::test]
async fn test_no_timeout_disables_step_timeout() {
    let workflow = Workflow::builder()
        .add_step("slow", SlowStep)
        .no_timeout()
        .build()
        .expect("valid workflow");

    assert!(workflow.run(&mut Context::new()).await.is_ok());
}

#[tokio::test]
async fn test_retry_and_timeout_combine() {
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
                Ok(Next::Done)
            })
        })
        .retry(RetryPolicy::fixed(1, Duration::from_millis(1)))
        .timeout(Duration::from_millis(20))
        .build()
        .expect("valid workflow");

    let report = workflow
        .run(&mut Context::new())
        .await
        .expect("workflow succeeds");

    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(report.total_retries(), 1);
}

#[test]
fn test_modifiers_require_a_step() {
    let cases = [
        (Workflow::builder().retry(RetryPolicy::none()), "retry"),
        (
            Workflow::builder().timeout(Duration::from_secs(1)),
            "timeout",
        ),
        (Workflow::builder().no_timeout(), "no_timeout"),
        (Workflow::builder().terminal(), "terminal"),
    ];
    for (builder, method) in cases {
        let result = builder.add_fn("a", |_ctx| Ok(Next::Done)).build();
        assert_eq!(result.unwrap_err(), BuildError::ModifierWithoutStep(method));
    }
}
