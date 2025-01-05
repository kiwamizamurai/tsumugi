use crate::context::Context;
use crate::error::WorkflowError;
use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct StepConfig {
    pub timeout: Option<Duration>,
    pub retries: u32,
    pub retry_delay: Duration,
}

impl Default for StepConfig {
    fn default() -> Self {
        Self {
            timeout: Some(Duration::from_secs(30)),
            retries: 3,
            retry_delay: Duration::from_secs(5),
        }
    }
}

#[async_trait]
pub trait Step<T>: Send + Sync {
    /// Execute the step and return the next step name (if any)
    async fn execute(&self, ctx: &mut Context<T>) -> Result<Option<String>, WorkflowError>;

    /// Return the step name
    fn name(&self) -> String {
        std::any::type_name::<Self>()
            .split("::")
            .last()
            .unwrap_or("UnknownStep")
            .to_string()
    }

    /// Return the step configuration
    fn config(&self) -> StepConfig {
        StepConfig::default()
    }

    /// Called when the step fails
    async fn on_failure(&self, _ctx: &mut Context<T>) -> Result<(), WorkflowError> {
        Ok(())
    }

    /// Called when the step succeeds
    async fn on_success(&self, _ctx: &mut Context<T>) -> Result<(), WorkflowError> {
        Ok(())
    }

    /// Format step information for debugging
    fn fmt_debug(&self) -> String {
        format!("Step '{}' (config: {:?})", self.name(), self.config())
    }

    /// Helper method to get the default name for a step type
    fn default_name() -> String
    where
        Self: Sized,
    {
        std::any::type_name::<Self>()
            .split("::")
            .last()
            .unwrap_or("UnknownStep")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define_step;
    use async_trait::async_trait;

    define_step!(TestStep);

    #[async_trait]
    impl Step<String> for TestStep {
        async fn execute(
            &self,
            ctx: &mut Context<String>,
        ) -> Result<Option<String>, WorkflowError> {
            ctx.insert("test", "executed".to_string());
            Ok(Some("next_step".to_string()))
        }
    }

    #[tokio::test]
    async fn test_step_execution() {
        let step = TestStep;
        let mut ctx = Context::new();

        let result = step.execute(&mut ctx).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("next_step".to_string()));
        assert_eq!(ctx.get("test").map(|s| s.as_str()), Some("executed"));
    }

    #[test]
    fn test_step_name() {
        let step = TestStep;
        assert_eq!(step.name(), "TestStep");
    }

    #[test]
    fn test_step_config() {
        let step = TestStep;
        let config = step.config();
        assert_eq!(config.retries, 3);
        assert_eq!(config.retry_delay, Duration::from_secs(5));
    }
}
