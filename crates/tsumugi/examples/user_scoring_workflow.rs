//! User scoring workflow demonstrating data processing pipeline.
//!
//! Demonstrates:
//! - Heterogeneous context (each type stored directly)
//! - Data validation
//! - Conditional logic based on computed values

use std::collections::HashMap;
use tsumugi::prelude::*;

// Data structures - stored directly without wrapper enum
#[derive(Debug, Clone)]
struct UserData {
    id: u64,
    name: String,
    age: u32,
}

#[derive(Debug, Clone)]
struct ProcessedData {
    user: UserData,
    score: f64,
    category: String,
}

// Step 1: Load user data
#[derive(Debug)]
struct UserDataLoadStep;

#[async_trait]
impl Step for UserDataLoadStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Loading user data...");

        let user = UserData {
            id: 1,
            name: "John Doe".to_string(),
            age: 30,
        };
        ctx.insert("user_data", user);

        Ok(Next::step("load_scores"))
    }
}

// Step 2: Load scores
#[derive(Debug)]
struct ScoresLoadStep;

#[async_trait]
impl Step for ScoresLoadStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Loading scores...");

        let scores: HashMap<u64, f64> = HashMap::from([(1, 85.5), (2, 92.0), (3, 78.3)]);
        ctx.insert("scores", scores);

        Ok(Next::step("validate"))
    }
}

// Step 3: Validate data
#[derive(Debug)]
struct DataValidationStep;

#[async_trait]
impl Step for DataValidationStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Validating data...");

        let user = ctx.require::<UserData>("user_data")?;

        if user.age < 18 {
            return Err("User must be 18 or older".into());
        }

        let scores = ctx.require::<HashMap<u64, f64>>("scores")?;

        if !scores.contains_key(&user.id) {
            return Err("Score not found for user".into());
        }

        Ok(Next::step("process"))
    }
}

// Step 4: Process data
#[derive(Debug)]
struct DataProcessingStep;

#[async_trait]
impl Step for DataProcessingStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Processing data...");

        let user = ctx.require::<UserData>("user_data")?.clone();

        let scores = ctx.require::<HashMap<u64, f64>>("scores")?;

        let score = scores
            .get(&user.id)
            .ok_or_else(|| "Score not found for user".to_string())?;

        let category = match *score {
            s if s >= 90.0 => "A",
            s if s >= 80.0 => "B",
            s if s >= 70.0 => "C",
            _ => "D",
        };

        let processed = ProcessedData {
            user,
            score: *score,
            category: category.to_string(),
        };

        ctx.insert("processed_data", processed);
        Ok(Next::step("notify"))
    }
}

// Step 5: Notification
#[derive(Debug)]
struct NotificationStep;

#[async_trait]
impl Step for NotificationStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        let processed = ctx.require::<ProcessedData>("processed_data")?;

        if processed.score < 80.0 {
            println!(
                "Notification: {} scored {} (Category {})",
                processed.user.name, processed.score, processed.category
            );
        }

        Ok(Next::Done)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = Workflow::builder()
        .add_step("load_user", UserDataLoadStep)
        .add_step("load_scores", ScoresLoadStep)
        .add_step("validate", DataValidationStep)
        .add_step("process", DataProcessingStep)
        .add_step("notify", NotificationStep)
        .start_with("load_user")
        .build()?;

    let mut ctx = Context::new();

    match workflow.run(&mut ctx).await {
        Ok(_) => {
            if let Some(processed) = ctx.get::<ProcessedData>("processed_data") {
                println!("\nWorkflow completed successfully");
                println!(
                    "Result: {} - Score: {}, Category: {}",
                    processed.user.name, processed.score, processed.category
                );
            }
        }
        Err(err) => {
            eprintln!("Workflow failed: {}", err);
            eprintln!("{}", err.report());
        }
    }

    Ok(())
}
