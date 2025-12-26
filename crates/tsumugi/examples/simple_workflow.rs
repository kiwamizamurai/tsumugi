//! Simple single-step workflow example.

use async_trait::async_trait;
use tsumugi::prelude::*;

#[derive(Debug)]
struct DataLoadStep;

#[async_trait]
impl Step for DataLoadStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Loading data...");
        ctx.insert("data", "sample data".to_string());
        Ok(StepOutput::done())
    }

    fn name(&self) -> StepName {
        StepName::new("DataLoadStep")
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = Workflow::builder()
        .add_step("load", DataLoadStep)
        .start_with("load")
        .build()?;

    let mut ctx = Context::new();

    match workflow.execute(&mut ctx).await {
        Ok(()) => {
            println!("Workflow completed successfully");
            if let Some(data) = ctx.get::<String>("data") {
                println!("Data: {}", data);
            }
        }
        Err(errors) => {
            for error in errors {
                eprintln!("Workflow failed: {:?}", error);
            }
        }
    }

    Ok(())
}
