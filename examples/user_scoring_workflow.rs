use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tsumugi::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserData {
    id: u64,
    name: String,
    age: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProcessedData {
    user: UserData,
    score: f64,
    category: String,
}

#[derive(Debug, Clone)]
enum WorkflowData {
    User(UserData),
    Scores(HashMap<u64, f64>),
    Processed(ProcessedData),
}

// Step 1: Load user data
define_step!(UserDataLoadStep);

#[async_trait]
impl Step<WorkflowData> for UserDataLoadStep {
    async fn execute(
        &self,
        ctx: &mut Context<WorkflowData>,
    ) -> Result<Option<StepName>, WorkflowError> {
        println!("Loading user data...");
        // Simulate loading from database
        let user = UserData {
            id: 1,
            name: "John Doe".to_string(),
            age: 30,
        };
        ctx.insert("user_data", WorkflowData::User(user));
        Ok(Some(StepName::new("AdditionalDataLoadStep")))
    }
}

// Step 2: Load additional data
define_step!(AdditionalDataLoadStep);

#[async_trait]
impl Step<WorkflowData> for AdditionalDataLoadStep {
    async fn execute(
        &self,
        ctx: &mut Context<WorkflowData>,
    ) -> Result<Option<StepName>, WorkflowError> {
        println!("Loading additional data...");
        // Simulate loading from external API
        let scores = HashMap::from([(1, 85.5), (2, 92.0), (3, 78.3)]);
        ctx.insert("scores", WorkflowData::Scores(scores));
        Ok(Some(StepName::new("DataValidationStep")))
    }
}

// Step 3: Validate data
define_step!(DataValidationStep);

#[async_trait]
impl Step<WorkflowData> for DataValidationStep {
    async fn execute(
        &self,
        ctx: &mut Context<WorkflowData>,
    ) -> Result<Option<StepName>, WorkflowError> {
        println!("Validating data...");

        let user = match ctx.get("user_data") {
            Some(WorkflowData::User(user)) => user,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "User data not found or invalid".to_string(),
                })
            }
        };

        if user.age < 18 {
            return Err(WorkflowError::StepError {
                step_name: self.name(),
                details: "User must be 18 or older".to_string(),
            });
        }

        let scores = match ctx.get("scores") {
            Some(WorkflowData::Scores(scores)) => scores,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Scores not found or invalid".to_string(),
                })
            }
        };

        if !scores.contains_key(&user.id) {
            return Err(WorkflowError::StepError {
                step_name: self.name(),
                details: "Score not found for user".to_string(),
            });
        }

        Ok(Some(StepName::new("DataProcessingStep")))
    }
}

// Step 4: Process data
define_step!(DataProcessingStep);

#[async_trait]
impl Step<WorkflowData> for DataProcessingStep {
    async fn execute(
        &self,
        ctx: &mut Context<WorkflowData>,
    ) -> Result<Option<StepName>, WorkflowError> {
        println!("Processing data...");

        let user = match ctx.get("user_data") {
            Some(WorkflowData::User(user)) => user,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "User data not found or invalid".to_string(),
                })
            }
        };

        let scores = match ctx.get("scores") {
            Some(WorkflowData::Scores(scores)) => scores,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Scores not found or invalid".to_string(),
                })
            }
        };

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
            user: user.clone(),
            score: *score,
            category: category.to_string(),
        };

        ctx.insert("processed_data", WorkflowData::Processed(processed));
        Ok(Some(StepName::new("NotificationStep")))
    }
}

// Step 5: Notification step (conditional)
define_step!(NotificationStep);

#[async_trait]
impl Step<WorkflowData> for NotificationStep {
    async fn execute(
        &self,
        ctx: &mut Context<WorkflowData>,
    ) -> Result<Option<StepName>, WorkflowError> {
        let processed = match ctx.get("processed_data") {
            Some(WorkflowData::Processed(processed)) => processed,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Processed data not found or invalid".to_string(),
                })
            }
        };

        if processed.score < 80.0 {
            println!(
                "Sending notification for low score: {} ({})",
                processed.user.name, processed.score
            );
        }

        Ok(None)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = Workflow::builder()
        .add::<UserDataLoadStep>()
        .add::<AdditionalDataLoadStep>()
        .add::<DataValidationStep>()
        .add::<DataProcessingStep>()
        .add::<NotificationStep>()
        .start_with_type::<UserDataLoadStep>()
        .build()?;

    let mut ctx = Context::new();

    match workflow.execute(&mut ctx).await {
        Ok(()) => {
            if let Some(WorkflowData::Processed(processed)) = ctx.get("processed_data") {
                println!("Workflow completed successfully");
                println!("Final result: {:?}", processed);
            }
        }
        Err(errors) => {
            for error in errors {
                println!("Workflow failed: {:?}", error);
            }
        }
    }

    Ok(())
}
