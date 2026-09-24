use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tsumugi::prelude::*;

const AMOUNT: Key<u64> = Key::new("amount");
const APPROVED: Key<bool> = Key::new("approved");

fn order_workflow() -> Result<Workflow, WorkflowError> {
    Workflow::builder()
        .add_fn("validate", |ctx| {
            let amount = ctx.get(AMOUNT).copied().unwrap_or_default();
            Ok(StepOutput::next(if amount > 0 {
                "charge"
            } else {
                "reject"
            }))
        })
        .then(["charge", "reject"])
        .add_fn("charge", |ctx| {
            ctx.insert(APPROVED, true);
            Ok(StepOutput::next("notify"))
        })
        .then(["notify"])
        .add_fn("reject", |ctx| {
            ctx.insert(APPROVED, false);
            Ok(StepOutput::next("notify"))
        })
        .then(["notify"])
        .add_fn("notify", |_ctx| Ok(StepOutput::done()))
        .terminal()
        .start_with("validate")
        .build()
}

#[tokio::test]
async fn test_typed_keys_across_steps() {
    let workflow = order_workflow().expect("valid workflow");

    let mut ctx = Context::new();
    ctx.insert(AMOUNT, 100);
    workflow.execute(&mut ctx).await.expect("workflow succeeds");
    assert_eq!(ctx.get(APPROVED), Some(&true));

    let mut ctx = Context::new();
    ctx.insert(AMOUNT, 0);
    workflow.execute(&mut ctx).await.expect("workflow succeeds");
    assert_eq!(ctx.get(APPROVED), Some(&false));
}

#[tokio::test]
async fn test_report_records_path_and_status() {
    let workflow = order_workflow().expect("valid workflow");
    let mut ctx = Context::new();
    ctx.insert(AMOUNT, 100);

    let report = workflow.execute(&mut ctx).await.expect("workflow succeeds");

    let path: Vec<&str> = report.path().map(StepName::as_str).collect();
    assert_eq!(path, ["validate", "charge", "notify"]);
    assert_eq!(
        report.steps()[0].status(),
        &StepStatus::Continued(StepName::new("charge"))
    );
    assert_eq!(report.steps()[2].status(), &StepStatus::Completed);
    assert_eq!(report.total_retries(), 0);
    assert!(report.duration() >= report.steps()[0].duration());
}

#[tokio::test]
async fn test_report_records_retries() {
    let attempts = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&attempts);
    let flaky = AsyncFnStep::new("flaky", move |_ctx| {
        let counter = Arc::clone(&counter);
        Box::pin(async move {
            if counter.fetch_add(1, Ordering::SeqCst) < 2 {
                return Err(WorkflowError::StepError {
                    step_name: StepName::new("flaky"),
                    details: "try again".to_string(),
                });
            }
            Ok(StepOutput::done())
        })
    });

    let workflow = Workflow::builder()
        .add_configured(
            "flaky",
            flaky,
            StepConfig {
                timeout: Some(Duration::from_secs(1)),
                retry_policy: RetryPolicy::fixed(5, Duration::from_millis(1)),
            },
        )
        .start_with("flaky")
        .build()
        .expect("valid workflow");

    let report = workflow
        .execute(&mut Context::new())
        .await
        .expect("workflow succeeds");

    assert_eq!(report.steps()[0].attempts(), 3);
    assert_eq!(report.total_retries(), 2);
}

#[tokio::test]
async fn test_failure_carries_partial_report() {
    let workflow = Workflow::builder()
        .add_fn("prepare", |_ctx| Ok(StepOutput::next("fail")))
        .add_fn("fail", |_ctx| {
            Err(WorkflowError::StepError {
                step_name: StepName::new("fail"),
                details: "boom".to_string(),
            })
        })
        .start_with("prepare")
        .build()
        .expect("valid workflow");

    let err = workflow.execute(&mut Context::new()).await.unwrap_err();

    assert!(matches!(err.error(), WorkflowError::StepError { .. }));
    assert_eq!(err.to_string(), "Step failed: fail, details: boom");
    let statuses: Vec<&StepStatus> = err.report().steps().iter().map(|r| r.status()).collect();
    assert_eq!(
        statuses,
        [
            &StepStatus::Continued(StepName::new("fail")),
            &StepStatus::Failed
        ]
    );

    let (error, report) = err.into_parts();
    assert!(matches!(error, WorkflowError::StepError { .. }));
    assert_eq!(report.steps().len(), 2);
}

#[tokio::test]
async fn test_undeclared_transition_is_rejected_at_runtime() {
    let workflow = Workflow::builder()
        .add_fn("a", |_ctx| Ok(StepOutput::next("c")))
        .then(["b"])
        .add_fn("b", |_ctx| Ok(StepOutput::done()))
        .add_fn("c", |_ctx| Ok(StepOutput::done()))
        .start_with("a")
        .build()
        .expect("valid workflow");

    let err = workflow.execute(&mut Context::new()).await.unwrap_err();

    assert!(matches!(
        err.error(),
        WorkflowError::UndeclaredTransition { from, to }
            if from.as_str() == "a" && to.as_str() == "c"
    ));
    // "c" never ran.
    assert_eq!(err.report().steps().len(), 1);
}

#[tokio::test]
async fn test_terminal_step_cannot_continue() {
    let workflow = Workflow::builder()
        .add_fn("a", |_ctx| Ok(StepOutput::next("a")))
        .terminal()
        .start_with("a")
        .build()
        .expect("valid workflow");

    let err = workflow.execute(&mut Context::new()).await.unwrap_err();
    assert!(matches!(
        err.error(),
        WorkflowError::UndeclaredTransition { .. }
    ));
}

#[tokio::test]
async fn test_execution_error_converts_with_question_mark() {
    async fn run(workflow: &Workflow) -> Result<(), WorkflowError> {
        workflow.execute(&mut Context::new()).await?;
        Ok(())
    }

    async fn run_boxed(workflow: &Workflow) -> Result<(), Box<dyn std::error::Error>> {
        workflow.execute(&mut Context::new()).await?;
        Ok(())
    }

    let workflow = Workflow::builder()
        .add_fn("a", |_ctx| Ok(StepOutput::next("missing")))
        .start_with("a")
        .build()
        .expect("valid workflow");

    assert!(matches!(
        run(&workflow).await,
        Err(WorkflowError::StepNotFound(_))
    ));
    assert!(run_boxed(&workflow).await.is_err());
}

#[test]
fn test_mermaid_output() {
    let expected = "\
flowchart TD
    __start((start))
    __end((end))
    s0[\"validate\"]
    s1[\"charge\"]
    s2[\"reject\"]
    s3[\"notify\"]
    __start --> s0
    s0 --> s1
    s0 --> s2
    s1 --> s3
    s2 --> s3
    s3 --> __end
";
    assert_eq!(
        order_workflow().expect("valid workflow").to_mermaid(),
        expected
    );
}
