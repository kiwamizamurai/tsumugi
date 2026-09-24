//! Workflows over user-defined state types.

use std::sync::Arc;
use tsumugi::prelude::*;
use tsumugi::StepStatus;

#[derive(Debug, Default)]
struct Order {
    total: u64,
    payment_id: Option<String>,
    shipped: bool,
    log: Vec<String>,
}

/// Capability trait implemented by any state that keeps a log.
trait HasLog: Send {
    fn log(&mut self) -> &mut Vec<String>;
}

impl HasLog for Order {
    fn log(&mut self) -> &mut Vec<String> {
        &mut self.log
    }
}

/// A step reusable with any state that has a log.
struct Audit(&'static str);

#[async_trait]
impl<S: HasLog> Step<S> for Audit {
    async fn run(&self, state: &mut S) -> StepResult {
        state.log().push(self.0.to_string());
        Ok(Next::step("charge"))
    }
}

struct Charge;

#[async_trait]
impl Step<Order> for Charge {
    async fn run(&self, order: &mut Order) -> StepResult {
        if order.total == 0 {
            return Err("nothing to charge".into());
        }
        order.payment_id = Some(format!("pay_{}", order.total));
        Ok(Next::step("ship"))
    }
}

fn order_workflow() -> Result<Workflow<Order>, tsumugi::BuildError> {
    WorkflowBuilder::<Order>::new()
        .add_step("audit", Audit("received"))
        .then(["charge"])
        .add_step("charge", Charge)
        .then(["ship"])
        .add_fn("ship", |order| {
            order.shipped = order.payment_id.is_some();
            Ok(Next::Done)
        })
        .terminal()
        .build()
}

#[tokio::test]
async fn test_custom_state_workflow() {
    let workflow = order_workflow().expect("valid workflow");
    let mut order = Order {
        total: 4_980,
        ..Order::default()
    };

    let report = workflow.run(&mut order).await.expect("workflow succeeds");

    assert_eq!(order.payment_id.as_deref(), Some("pay_4980"));
    assert!(order.shipped);
    assert_eq!(order.log, ["received"]);
    assert_eq!(
        report.steps().last().map(|r| r.status()),
        Some(&StepStatus::Completed)
    );
}

#[tokio::test]
async fn test_custom_state_failure() {
    let workflow = order_workflow().expect("valid workflow");
    let mut order = Order::default();

    let err = workflow.run(&mut order).await.unwrap_err();

    assert_eq!(err.step(), "charge");
    assert_eq!(err.to_string(), "step 'charge' failed: nothing to charge");
    assert!(!order.shipped);
}

#[tokio::test]
async fn test_custom_state_async_closure() {
    let workflow = WorkflowBuilder::<Order>::new()
        .add_async_fn("charge", |order| {
            Box::pin(async move {
                tokio::task::yield_now().await;
                order.payment_id = Some("pay_async".to_string());
                Ok(Next::Done)
            })
        })
        .build()
        .expect("valid workflow");

    let mut order = Order::default();
    workflow.run(&mut order).await.expect("workflow succeeds");

    assert_eq!(order.payment_id.as_deref(), Some("pay_async"));
}

#[tokio::test]
async fn test_custom_state_workflow_can_be_spawned() {
    let workflow = Arc::new(order_workflow().expect("valid workflow"));

    let handle = tokio::spawn(async move {
        let mut order = Order {
            total: 1,
            ..Order::default()
        };
        workflow.run(&mut order).await.map(|_| order)
    });

    let order = handle
        .await
        .expect("task panicked")
        .expect("workflow failed");
    assert!(order.shipped);
}
