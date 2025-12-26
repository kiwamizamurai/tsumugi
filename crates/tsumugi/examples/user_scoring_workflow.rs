//! User scoring workflow demonstrating data processing pipeline.
//!
//! Demonstrates:
//! - Heterogeneous context (each type stored directly)
//! - Data validation
//! - Conditional logic based on computed values

use async_trait::async_trait;
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
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Loading user data...");

        let user = UserData {
            id: 1,
            name: "John Doe".to_string(),
            age: 30,
        };
        ctx.insert("user_data", user);

        Ok(StepOutput::next("load_scores"))
    }

    fn name(&self) -> StepName {
        StepName::new("UserDataLoad")
    }
}

// Step 2: Load scores
#[derive(Debug)]
struct ScoresLoadStep;

#[async_trait]
impl Step for ScoresLoadStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Loading scores...");

        let scores: HashMap<u64, f64> = HashMap::from([(1, 85.5), (2, 92.0), (3, 78.3)]);
        ctx.insert("scores", scores);

        Ok(StepOutput::next("validate"))
    }

    fn name(&self) -> StepName {
        StepName::new("ScoresLoad")
    }
}

// Step 3: Validate data
#[derive(Debug)]
struct DataValidationStep;

#[async_trait]
impl Step for DataValidationStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Validating data...");

        let user = ctx
            .get::<UserData>("user_data")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "User data not found".to_string(),
            })?;

        if user.age < 18 {
            return Err(WorkflowError::StepError {
                step_name: self.name(),
                details: "User must be 18 or older".to_string(),
            });
        }

        let scores = ctx
            .get::<HashMap<u64, f64>>("scores")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Scores not found".to_string(),
            })?;

        if !scores.contains_key(&user.id) {
            return Err(WorkflowError::StepError {
                step_name: self.name(),
                details: "Score not found for user".to_string(),
            });
        }

        Ok(StepOutput::next("process"))
    }

    fn name(&self) -> StepName {
        StepName::new("DataValidation")
    }
}

// Step 4: Process data
#[derive(Debug)]
struct DataProcessingStep;

#[async_trait]
impl Step for DataProcessingStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Processing data...");

        let user = ctx
            .get::<UserData>("user_data")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "User data not found".to_string(),
            })?
            .clone();

        let scores = ctx
            .get::<HashMap<u64, f64>>("scores")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Scores not found".to_string(),
            })?;

        let score = scores
            .get(&user.id)
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Score not found for user".to_string(),
            })?;

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
        Ok(StepOutput::next("notify"))
    }

    fn name(&self) -> StepName {
        StepName::new("DataProcessing")
    }
}

// Step 5: Notification
#[derive(Debug)]
struct NotificationStep;

#[async_trait]
impl Step for NotificationStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        let processed =
            ctx.get::<ProcessedData>("processed_data")
                .ok_or_else(|| WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Processed data not found".to_string(),
                })?;

        if processed.score < 80.0 {
            println!(
                "Notification: {} scored {} (Category {})",
                processed.user.name, processed.score, processed.category
            );
        }

        Ok(StepOutput::done())
    }

    fn name(&self) -> StepName {
        StepName::new("Notification")
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

    match workflow.execute(&mut ctx).await {
        Ok(()) => {
            if let Some(processed) = ctx.get::<ProcessedData>("processed_data") {
                println!("\nWorkflow completed successfully");
                println!(
                    "Result: {} - Score: {}, Category: {}",
                    processed.user.name, processed.score, processed.category
                );
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
