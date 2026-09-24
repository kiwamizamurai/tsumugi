//! Simple single-step workflow example.

use tsumugi::prelude::*;

#[derive(Debug)]
struct DataLoadStep;

#[async_trait]
impl Step for DataLoadStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Loading data...");
        ctx.insert("data", "sample data".to_string());
        Ok(Next::Done)
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

    match workflow.run(&mut ctx).await {
        Ok(_) => {
            println!("Workflow completed successfully");
            if let Some(data) = ctx.get::<String>("data") {
                println!("Data: {}", data);
            }
        }
        Err(err) => {
            eprintln!("Workflow failed: {}", err);
            eprintln!("{}", err.report());
        }
    }

    Ok(())
}
