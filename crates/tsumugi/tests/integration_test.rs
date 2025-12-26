use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tsumugi::prelude::*;

#[derive(Debug)]
struct Step1;

#[async_trait]
impl Step for Step1 {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        ctx.insert("step1", "completed".to_string());
        Ok(StepOutput::next("step2"))
    }

    fn name(&self) -> StepName {
        StepName::new("Step1")
    }
}

#[derive(Debug)]
struct Step2;

#[async_trait]
impl Step for Step2 {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        ctx.insert("step2", "completed".to_string());
        Ok(StepOutput::done())
    }

    fn name(&self) -> StepName {
        StepName::new("Step2")
    }
}

#[tokio::test]
async fn test_complete_workflow() {
    let workflow = Workflow::builder()
        .add_step("step1", Step1)
        .add_step("step2", Step2)
        .start_with("step1")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_ok());
    assert_eq!(
        ctx.get::<String>("step1").map(|s| s.as_str()),
        Some("completed")
    );
    assert_eq!(
        ctx.get::<String>("step2").map(|s| s.as_str()),
        Some("completed")
    );
}

#[derive(Debug)]
struct StepWithInvalidNext;

#[async_trait]
impl Step for StepWithInvalidNext {
    async fn execute(&self, _ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        Ok(StepOutput::next("nonexistent_step"))
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
        .expect("valid workflow");

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
        matches!(&errors[0], WorkflowError::StepNotFound(name) if name.as_str() == "nonexistent_step")
    );
}

#[derive(Debug)]
struct SlowStep;

#[async_trait]
impl Step for SlowStep {
    async fn execute(&self, _ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        tokio::time::sleep(Duration::from_secs(10)).await;
        Ok(StepOutput::done())
    }

    fn name(&self) -> StepName {
        StepName::new("SlowStep")
    }
}

#[tokio::test]
async fn test_timeout_error() {
    let workflow = Workflow::builder()
        .add_with_timeout("slow", SlowStep, Duration::from_millis(50))
        .start_with("slow")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
        matches!(&errors[0], WorkflowError::Timeout { step_name } if step_name.as_str() == "SlowStep")
    );
}

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
impl Step for RetryableStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt < self.fail_until {
            Err(WorkflowError::StepError {
                step_name: self.name(),
                details: format!("Attempt {} failed", attempt + 1),
            })
        } else {
            ctx.insert("success", true);
            Ok(StepOutput::done())
        }
    }

    fn name(&self) -> StepName {
        StepName::new("RetryableStep")
    }
}

impl Retryable for RetryableStep {
    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::fixed(3, Duration::from_millis(10))
    }
}

#[tokio::test]
async fn test_retry_eventual_success() {
    let attempts = Arc::new(AtomicU32::new(0));
    let step = RetryableStep {
        attempts: attempts.clone(),
        fail_until: 2,
    };

    let workflow = Workflow::builder()
        .add_retryable("retry", step)
        .start_with("retry")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_ok());
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    assert_eq!(ctx.get::<bool>("success"), Some(&true));
}

#[tokio::test]
async fn test_retry_exhausted() {
    let attempts = Arc::new(AtomicU32::new(0));
    let step = RetryableStep {
        attempts: attempts.clone(),
        fail_until: 10,
    };

    let workflow = Workflow::builder()
        .add_retryable("retry", step)
        .start_with("retry")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    let result = workflow.execute(&mut ctx).await;

    assert!(result.is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn test_heterogeneous_context() {
    #[derive(Debug)]
    struct MultiTypeStep;

    #[async_trait]
    impl Step for MultiTypeStep {
        async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
            ctx.insert("int_val", 42i32);
            ctx.insert("str_val", "hello".to_string());
            ctx.insert("bool_val", true);
            Ok(StepOutput::done())
        }

        fn name(&self) -> StepName {
            StepName::new("MultiTypeStep")
        }
    }

    let workflow = Workflow::builder()
        .add_step("multi", MultiTypeStep)
        .start_with("multi")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    workflow.execute(&mut ctx).await.expect("workflow failed");

    assert_eq!(ctx.get::<i32>("int_val"), Some(&42));
    assert_eq!(ctx.get::<String>("str_val"), Some(&"hello".to_string()));
    assert_eq!(ctx.get::<bool>("bool_val"), Some(&true));

    // Wrong type returns None
    assert_eq!(ctx.get::<String>("int_val"), None);
}
