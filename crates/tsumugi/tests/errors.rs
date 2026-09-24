//! Error ergonomics: `?` in steps, error sources and conversions.

use std::error::Error;
use std::fmt;
use tsumugi::prelude::*;
use tsumugi::{ErrorKind, Failure, MissingValue};

#[derive(Debug)]
struct GatewayError {
    code: u16,
}

impl fmt::Display for GatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gateway returned {}", self.code)
    }
}

impl Error for GatewayError {}

fn charge(amount: u64) -> Result<String, GatewayError> {
    if amount > 1_000 {
        Err(GatewayError { code: 402 })
    } else {
        Ok(format!("pay_{}", amount))
    }
}

fn workflow() -> Result<Workflow, tsumugi::BuildError> {
    Workflow::builder()
        .add_fn("charge", |ctx| {
            let amount = *ctx.require::<u64>("amount")?; // MissingValue
            let payment = charge(amount)?; // GatewayError
            ctx.insert("payment", payment);
            Ok(Next::Done)
        })
        .build()
}

#[tokio::test]
async fn test_custom_error_is_preserved() {
    let mut ctx = Context::new();
    ctx.insert("amount", 5_000u64);

    let err = workflow()
        .expect("valid workflow")
        .run(&mut ctx)
        .await
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "step 'charge' failed: gateway returned 402"
    );

    // The original error is reachable both through `kind` and `source`.
    let ErrorKind::Step(Failure::Error(step_error)) = err.kind() else {
        panic!("unexpected error kind: {:?}", err.kind());
    };
    assert_eq!(
        step_error.downcast_ref::<GatewayError>().map(|e| e.code),
        Some(402)
    );

    let source = err.source().expect("error has a source");
    assert_eq!(
        source.downcast_ref::<GatewayError>().map(|e| e.code),
        Some(402)
    );
}

#[tokio::test]
async fn test_missing_context_value_is_reported() {
    let err = workflow()
        .expect("valid workflow")
        .run(&mut Context::new())
        .await
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "step 'charge' failed: context has no value of type `u64` for key `amount`"
    );
    let missing = err
        .source()
        .and_then(|source| source.downcast_ref::<MissingValue>())
        .expect("source is MissingValue");
    assert_eq!(missing.key(), "amount");
}

#[tokio::test]
async fn test_execution_error_works_with_question_mark() {
    async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
        workflow()
            .expect("valid workflow")
            .run(&mut Context::new())
            .await?;
        Ok(())
    }

    assert!(run().await.is_err());
}

#[test]
fn test_build_error_works_with_question_mark() {
    fn build() -> Result<Workflow, Box<dyn Error + Send + Sync>> {
        Ok(Workflow::builder().build()?)
    }

    let err = build().unwrap_err();
    assert_eq!(err.to_string(), "workflow has no steps");
}

#[test]
fn test_execution_error_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync + 'static>() {}
    assert_send_sync::<tsumugi::ExecutionError>();
    assert_send_sync::<tsumugi::BuildError>();
    assert_send_sync::<StepError>();
}
