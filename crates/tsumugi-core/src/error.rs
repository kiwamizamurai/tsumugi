//! Workflow error types.

use crate::step::StepName;
use thiserror::Error;

/// The type of lifecycle hook that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookType {
    /// The `on_success` hook.
    OnSuccess,
    /// The `on_failure` hook.
    OnFailure,
}

impl std::fmt::Display for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookType::OnSuccess => write!(f, "on_success"),
            HookType::OnFailure => write!(f, "on_failure"),
        }
    }
}

/// Errors that can occur during workflow execution.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum WorkflowError {
    /// A step failed during execution.
    #[error("Step failed: {step_name}, details: {details}")]
    StepError {
        /// The name of the step that failed.
        step_name: StepName,
        /// Details about the failure.
        details: String,
    },

    /// A step exceeded its timeout duration.
    #[error("Timeout occurred in step: {step_name}")]
    Timeout {
        /// The name of the step that timed out.
        step_name: StepName,
    },

    /// A referenced step was not found in the workflow.
    #[error("Step not found: {0}")]
    StepNotFound(StepName),

    /// The workflow configuration is invalid.
    #[error("Invalid workflow configuration: {0}")]
    Configuration(String),

    /// A lifecycle hook failed.
    #[error("Hook '{hook_type}' failed in step '{step_name}': {details}")]
    HookError {
        /// The name of the step whose hook failed.
        step_name: StepName,
        /// Which hook failed.
        hook_type: HookType,
        /// Details about the failure.
        details: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = WorkflowError::StepError {
            step_name: StepName::new("test_step"),
            details: "test error".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Step failed: test_step, details: test error"
        );
    }

    #[test]
    fn test_hook_type_display() {
        assert_eq!(HookType::OnSuccess.to_string(), "on_success");
        assert_eq!(HookType::OnFailure.to_string(), "on_failure");
    }
}
