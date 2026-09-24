//! User scoring workflow demonstrating data processing pipeline.
//!
//! Demonstrates:
//! - A dedicated state struct filled in step by step
//! - Data validation
//! - Conditional logic based on computed values

use std::collections::HashMap;
use tsumugi::prelude::*;

// Data structures
#[derive(Debug)]
struct UserData {
    id: u64,
    name: String,
    age: u32,
}

#[derive(Debug)]
struct ProcessedData {
    score: f64,
    category: String,
}

/// The state shared by all steps. Each field is produced by a step, so it is
/// `None` until that step has run.
#[derive(Default)]
struct ScoringState {
    user: Option<UserData>,
    scores: Option<HashMap<u64, f64>>,
    processed: Option<ProcessedData>,
}

// Step 1: Load user data
struct UserDataLoadStep;

#[async_trait]
impl Step<ScoringState> for UserDataLoadStep {
    async fn run(&self, state: &mut ScoringState) -> StepResult {
        println!("Loading user data...");

        state.user = Some(UserData {
            id: 1,
            name: "John Doe".to_string(),
            age: 30,
        });

        Ok(Next::step("load_scores"))
    }
}

// Step 2: Load scores
struct ScoresLoadStep;

#[async_trait]
impl Step<ScoringState> for ScoresLoadStep {
    async fn run(&self, state: &mut ScoringState) -> StepResult {
        println!("Loading scores...");

        state.scores = Some(HashMap::from([(1, 85.5), (2, 92.0), (3, 78.3)]));

        Ok(Next::step("validate"))
    }
}

// Step 3: Validate data
struct DataValidationStep;

#[async_trait]
impl Step<ScoringState> for DataValidationStep {
    async fn run(&self, state: &mut ScoringState) -> StepResult {
        println!("Validating data...");

        let user = state.user.as_ref().ok_or("User data not loaded")?;

        if user.age < 18 {
            return Err("User must be 18 or older".into());
        }

        let scores = state.scores.as_ref().ok_or("Scores not loaded")?;

        if !scores.contains_key(&user.id) {
            return Err("Score not found for user".into());
        }

        Ok(Next::step("process"))
    }
}

// Step 4: Process data
struct DataProcessingStep;

#[async_trait]
impl Step<ScoringState> for DataProcessingStep {
    async fn run(&self, state: &mut ScoringState) -> StepResult {
        println!("Processing data...");

        let user = state.user.as_ref().ok_or("User data not loaded")?;
        let scores = state.scores.as_ref().ok_or("Scores not loaded")?;

        let score = *scores.get(&user.id).ok_or("Score not found for user")?;

        let category = match score {
            s if s >= 90.0 => "A",
            s if s >= 80.0 => "B",
            s if s >= 70.0 => "C",
            _ => "D",
        };

        state.processed = Some(ProcessedData {
            score,
            category: category.to_string(),
        });

        Ok(Next::step("notify"))
    }
}

// Step 5: Notification
struct NotificationStep;

#[async_trait]
impl Step<ScoringState> for NotificationStep {
    async fn run(&self, state: &mut ScoringState) -> StepResult {
        let user = state.user.as_ref().ok_or("User data not loaded")?;
        let processed = state.processed.as_ref().ok_or("Data not processed")?;

        if processed.score < 80.0 {
            println!(
                "Notification: {} scored {} (Category {})",
                user.name, processed.score, processed.category
            );
        }

        Ok(Next::Done)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = WorkflowBuilder::<ScoringState>::new()
        .add_step("load_user", UserDataLoadStep)
        .then(["load_scores"])
        .add_step("load_scores", ScoresLoadStep)
        .then(["validate"])
        .add_step("validate", DataValidationStep)
        .then(["process"])
        .add_step("process", DataProcessingStep)
        .then(["notify"])
        .add_step("notify", NotificationStep)
        .terminal()
        .build()?;

    let mut state = ScoringState::default();

    match workflow.run(&mut state).await {
        Ok(_) => {
            let user = state.user.as_ref().ok_or("User data not loaded")?;
            let processed = state.processed.as_ref().ok_or("Data not processed")?;
            println!("\nWorkflow completed successfully");
            println!(
                "Result: {} - Score: {}, Category: {}",
                user.name, processed.score, processed.category
            );
        }
        Err(err) => {
            eprintln!("Workflow failed: {}", err);
            eprintln!("{}", err.report());
        }
    }

    Ok(())
}
