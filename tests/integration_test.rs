use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tsumugi::prelude::*;
use tsumugi::{Context, Step, StepConfig, Workflow, WorkflowError};

#[derive(Debug)]
struct Step1 {
    config: StepConfig,
}

#[async_trait]
impl Step<String> for Step1 {
    async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
        ctx.insert("step1", "completed".to_string());
        Ok(Some(StepName::new("step2")))
    }

    fn name(&self) -> StepName {
        StepName::new("Step1")
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
    async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
        ctx.insert("step2", "completed".to_string());
        Ok(None)
    }

    fn name(&self) -> StepName {
        StepName::new("Step2")
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

// Test for step not found error
#[derive(Debug)]
struct StepWithInvalidNext;

#[async_trait]
impl Step<String> for StepWithInvalidNext {
    async fn execute(&self, _ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
        Ok(Some(StepName::new("nonexistent_step")))
    }

    fn name(&self) -> StepName {
        StepName::new("StepWithInvalidNext")
    }
}

#[tokio::test]
async fn test_step_not_found_error() {
    let workflow = Workflow::builder()
        .add_step("start", StepWithInvalidNext)
        .start_with("start")
        .build()
        .unwrap();

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
        matches!(&errors[0], WorkflowError::StepNotFound(name) if name.as_str() == "nonexistent_step")
    );
}

// Test for timeout
#[derive(Debug)]
struct SlowStep;

#[async_trait]
impl Step<String> for SlowStep {
    async fn execute(&self, _ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
        tokio::time::sleep(Duration::from_secs(10)).await;
        Ok(None)
    }

    fn name(&self) -> StepName {
        StepName::new("SlowStep")
    }

    fn config(&self) -> StepConfig {
        StepConfig {
            timeout: Some(Duration::from_millis(50)),
            retry_policy: RetryPolicy::None,
        }
    }
}

#[tokio::test]
async fn test_timeout_error() {
    let workflow = Workflow::builder()
        .add_step("slow", SlowStep)
        .start_with("slow")
        .build()
        .unwrap();

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
        matches!(&errors[0], WorkflowError::Timeout { step_name } if step_name.as_str() == "SlowStep")
    );
}

// Test for hook error propagation
#[derive(Debug)]
struct StepWithFailingHook;

#[async_trait]
impl Step<String> for StepWithFailingHook {
    async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
        ctx.insert("executed", "true".to_string());
        Ok(None)
    }

    fn name(&self) -> StepName {
        StepName::new("StepWithFailingHook")
    }

    async fn on_success(&self, _ctx: &mut Context<String>) -> Result<(), WorkflowError> {
        Err(WorkflowError::StepError {
            step_name: self.name(),
            details: "Hook failed intentionally".to_string(),
        })
    }
}

#[tokio::test]
async fn test_hook_error_propagation() {
    let workflow = Workflow::builder()
        .add_step("hook_test", StepWithFailingHook)
        .start_with("hook_test")
        .build()
        .unwrap();

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    // Workflow should still complete but with hook error
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(
        &errors[0],
        WorkflowError::HookError {
            hook_type: HookType::OnSuccess,
            ..
        }
    ));
    // Step should have executed
    assert_eq!(ctx.get("executed").map(|s| s.as_str()), Some("true"));
}

// Test for retry with eventual success
struct RetryableStep {
    attempts: Arc<AtomicU32>,
    fail_until: u32,
}

impl std::fmt::Debug for RetryableStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetryableStep").finish()
    }
}

#[async_trait]
impl Step<String> for RetryableStep {
    async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt < self.fail_until {
            Err(WorkflowError::StepError {
                step_name: self.name(),
                details: format!("Attempt {} failed", attempt + 1),
            })
        } else {
            ctx.insert("success", "true".to_string());
            Ok(None)
        }
    }

    fn name(&self) -> StepName {
        StepName::new("RetryableStep")
    }

    fn config(&self) -> StepConfig {
        StepConfig {
            timeout: Some(Duration::from_secs(30)),
            retry_policy: RetryPolicy::fixed(3, Duration::from_millis(10)),
        }
    }
}

#[tokio::test]
async fn test_retry_eventual_success() {
    let attempts = Arc::new(AtomicU32::new(0));
    let step = RetryableStep {
        attempts: attempts.clone(),
        fail_until: 2, // Fail twice, succeed on third attempt
    };

    let workflow = Workflow::builder()
        .add_step("retry", step)
        .start_with("retry")
        .build()
        .unwrap();

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_ok());
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    assert_eq!(ctx.get("success").map(|s| s.as_str()), Some("true"));
}

#[tokio::test]
async fn test_retry_exhausted() {
    let attempts = Arc::new(AtomicU32::new(0));
    let step = RetryableStep {
        attempts: attempts.clone(),
        fail_until: 10, // Always fail
    };

    let workflow = Workflow::builder()
        .add_step("retry", step)
        .start_with("retry")
        .build()
        .unwrap();

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_err());
    // 1 initial + 3 retries = 4 total attempts
    assert_eq!(attempts.load(Ordering::SeqCst), 4);
}
