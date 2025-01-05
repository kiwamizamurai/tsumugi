use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("Step failed: {step_name}, details: {details}")]
    StepError { step_name: String, details: String },

    #[error("Timeout occurred in step: {step_name}")]
    Timeout { step_name: String },

    #[error("Step not found: {0}")]
    StepNotFound(String),

    #[error("Invalid workflow configuration: {0}")]
    Configuration(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = WorkflowError::StepError {
            step_name: "test_step".to_string(),
            details: "test error".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Step failed: test_step, details: test error"
        );

        let timeout_error = WorkflowError::Timeout {
            step_name: "test_step".to_string(),
        };
        assert_eq!(
            timeout_error.to_string(),
            "Timeout occurred in step: test_step"
        );
    }
}
