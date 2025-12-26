use async_trait::async_trait;
use tsumugi::prelude::*;

define_step!(DataLoadStep);

#[async_trait]
impl Step<String> for DataLoadStep {
    async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
        println!("Loading data...");
        ctx.insert("data", "sample data".to_string());
        Ok(None)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = Workflow::builder()
        .add::<DataLoadStep>()
        .start_with_type::<DataLoadStep>()
        .build()?;

    let mut ctx = Context::new();

    match workflow.execute(&mut ctx).await {
        Ok(()) => println!("Workflow completed successfully"),
        Err(errors) => {
            for error in errors {
                println!("Workflow failed: {:?}", error);
            }
        }
    }

    Ok(())
}
