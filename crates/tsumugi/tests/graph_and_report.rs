//! Typed keys, declared transitions, Mermaid output and execution reports.

use tsumugi::prelude::*;
use tsumugi::{BuildError, ErrorKind, StepStatus};

const AMOUNT: Key<u64> = Key::new("amount");
const APPROVED: Key<bool> = Key::new("approved");

fn order_workflow() -> Result<Workflow, BuildError> {
    Workflow::builder()
        .add_fn("validate", |ctx| {
            let amount = *ctx.require(AMOUNT)?;
            Ok(Next::step(if amount > 0 { "charge" } else { "reject" }))
        })
        .then(["charge", "reject"])
        .add_fn("charge", |ctx| {
            ctx.insert(APPROVED, true);
            Ok(Next::step("notify"))
        })
        .then(["notify"])
        .add_fn("reject", |ctx| {
            ctx.insert(APPROVED, false);
            Ok(Next::step("notify"))
        })
        .then(["notify"])
        .add_fn("notify", |_ctx| Ok(Next::Done))
        .terminal()
        .build()
}

#[tokio::test]
async fn test_typed_keys_across_steps() {
    let workflow = order_workflow().expect("valid workflow");

    let mut ctx = Context::new();
    ctx.insert(AMOUNT, 100);
    workflow.run(&mut ctx).await.expect("workflow succeeds");
    assert_eq!(ctx.get(APPROVED), Some(&true));

    let mut ctx = Context::new();
    ctx.insert(AMOUNT, 0);
    workflow.run(&mut ctx).await.expect("workflow succeeds");
    assert_eq!(ctx.get(APPROVED), Some(&false));
}

#[tokio::test]
async fn test_report_records_path_and_status() {
    let workflow = order_workflow().expect("valid workflow");
    let mut ctx = Context::new();
    ctx.insert(AMOUNT, 100);

    let report = workflow.run(&mut ctx).await.expect("workflow succeeds");

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
async fn test_failure_carries_partial_report() {
    let workflow = Workflow::builder()
        .add_fn("prepare", |_ctx| Ok(Next::step("fail")))
        .add_fn("fail", |_ctx| Err("boom".into()))
        .build()
        .expect("valid workflow");

    let err = workflow.run(&mut Context::new()).await.unwrap_err();

    let statuses: Vec<&StepStatus> = err.report().steps().iter().map(|r| r.status()).collect();
    assert_eq!(
        statuses,
        [
            &StepStatus::Continued(StepName::new("fail")),
            &StepStatus::Failed
        ]
    );

    let (step, _kind, report) = err.into_parts();
    assert_eq!(step, "fail");
    assert_eq!(report.steps().len(), 2);
}

#[tokio::test]
async fn test_undeclared_transition_is_rejected_at_runtime() {
    let workflow = Workflow::builder()
        .add_fn("a", |_ctx| Ok(Next::step("c")))
        .then(["b"])
        .add_fn("b", |_ctx| Ok(Next::Done))
        .add_fn("c", |_ctx| Ok(Next::Done))
        .build()
        .expect("valid workflow");

    let err = workflow.run(&mut Context::new()).await.unwrap_err();

    assert_eq!(err.step(), "a");
    assert!(matches!(err.kind(), ErrorKind::UndeclaredTransition(to) if to == "c"));
    // "c" never ran.
    assert_eq!(err.report().steps().len(), 1);
}

#[tokio::test]
async fn test_terminal_step_cannot_continue() {
    let workflow = Workflow::builder()
        .add_fn("a", |_ctx| Ok(Next::step("a")))
        .terminal()
        .build()
        .expect("valid workflow");

    let err = workflow.run(&mut Context::new()).await.unwrap_err();
    assert!(matches!(err.kind(), ErrorKind::UndeclaredTransition(_)));
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
    let workflow = order_workflow().expect("valid workflow");
    assert_eq!(workflow.to_mermaid(), expected);
}
