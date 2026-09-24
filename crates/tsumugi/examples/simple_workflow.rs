//! Simple single-step workflow example.

use tsumugi::prelude::*;

const DATA: Key<String> = Key::new("data");

struct DataLoadStep;

#[async_trait]
impl Step for DataLoadStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Loading data...");
        ctx.insert(DATA, "sample data".to_string());
        Ok(Next::Done)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = Workflow::builder().add_step("load", DataLoadStep).build()?;

    let mut ctx = Context::new();
    workflow.run(&mut ctx).await?;

    println!("Workflow completed successfully");
    println!("Data: {}", ctx.require(DATA)?);
    Ok(())
}
