use async_trait::async_trait;
use tsumugi::prelude::*;
use tsumugi::{Context, Step, StepConfig, Workflow, WorkflowError};

#[derive(Debug)]
struct Step1 {
    config: StepConfig,
}

#[async_trait]
impl Step<String> for Step1 {
    async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<String>, WorkflowError> {
        ctx.insert("step1", "completed".to_string());
        Ok(Some("step2".to_string()))
    }

    fn name(&self) -> String {
        "Step1".to_string()
    }

    fn config(&self) -> StepConfig {
        self.config.clone()
    }
}

#[derive(Debug)]
struct Step2 {
    config: StepConfig,
}

#[async_trait]
impl Step<String> for Step2 {
    async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<String>, WorkflowError> {
        ctx.insert("step2", "completed".to_string());
        Ok(None)
    }

    fn name(&self) -> String {
        "Step2".to_string()
    }

    fn config(&self) -> StepConfig {
        self.config.clone()
    }
}

#[tokio::test]
async fn test_complete_workflow() {
    let workflow = Workflow::builder()
        .add_step(
            "step1",
            Step1 {
                config: StepConfig::default(),
            },
        )
        .add_step(
            "step2",
            Step2 {
                config: StepConfig::default(),
            },
        )
        .start_with("step1")
        .build()
        .unwrap();

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_ok());
    assert_eq!(ctx.get("step1").map(|s| s.as_str()), Some("completed"));
    assert_eq!(ctx.get("step2").map(|s| s.as_str()), Some("completed"));
}
