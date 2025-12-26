use crate::step::StepName;
use thiserror::Error;

/// The type of lifecycle hook that failed.
///
/// Used in [`WorkflowError::HookError`] to identify which hook caused the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookType {
    /// The `on_success` hook, called after a step completes successfully.
    OnSuccess,
    /// The `on_failure` hook, called after a step fails.
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
///
/// This enum represents all possible error conditions when running a workflow.
/// Each variant provides contextual information about what went wrong.
///
/// # Non-Exhaustive
///
/// This enum is marked `#[non_exhaustive]` to allow adding new variants
/// in future versions without breaking downstream code. When matching
/// on this error, always include a wildcard pattern:
///
/// ```
/// use tsumugi::{WorkflowError, StepName};
///
/// fn handle_error(error: WorkflowError) {
///     match error {
///         WorkflowError::StepError { step_name, details } => {
///             eprintln!("Step {} failed: {}", step_name, details);
///         }
///         WorkflowError::Timeout { step_name } => {
///             eprintln!("Step {} timed out", step_name);
///         }
///         WorkflowError::StepNotFound(name) => {
///             eprintln!("Step {} not found", name);
///         }
///         WorkflowError::Configuration(msg) => {
///             eprintln!("Configuration error: {}", msg);
///         }
///         WorkflowError::HookError { step_name, hook_type, details } => {
///             eprintln!("Hook {} failed in {}: {}", hook_type, step_name, details);
///         }
///         _ => eprintln!("Unknown error: {}", error),
///     }
/// }
/// ```
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum WorkflowError {
    /// A step failed during execution.
    ///
    /// This error is returned when a step's `execute` method returns an error
    /// and all retry attempts have been exhausted.
    #[error("Step failed: {step_name}, details: {details}")]
    StepError {
        /// The name of the step that failed
        step_name: StepName,
        /// Details about the failure
        details: String,
    },

    /// A step exceeded its timeout duration.
    ///
    /// This error is returned when a step takes longer than its configured
    /// timeout and all retry attempts have been exhausted.
    #[error("Timeout occurred in step: {step_name}")]
    Timeout {
        /// The name of the step that timed out
        step_name: StepName,
    },

    /// A referenced step was not found in the workflow.
    ///
    /// This error occurs when:
    /// - The start step specified in `build()` doesn't exist
    /// - A step returns a `StepName` that isn't registered
    #[error("Step not found: {0}")]
    StepNotFound(StepName),

    /// The workflow configuration is invalid.
    ///
    /// This error is returned by the builder when required configuration
    /// is missing or invalid.
    #[error("Invalid workflow configuration: {0}")]
    Configuration(String),

    /// A lifecycle hook failed.
    ///
    /// This error occurs when `on_success` or `on_failure` returns an error.
    #[error("Hook '{hook_type}' failed in step '{step_name}': {details}")]
    HookError {
        /// The name of the step whose hook failed
        step_name: StepName,
        /// Which hook failed
        hook_type: HookType,
        /// Details about the failure
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

        let timeout_error = WorkflowError::Timeout {
            step_name: StepName::new("test_step"),
        };
        assert_eq!(
            timeout_error.to_string(),
            "Timeout occurred in step: test_step"
        );
    }

    #[test]
    fn test_hook_error_display() {
        let error = WorkflowError::HookError {
            step_name: StepName::new("test_step"),
            hook_type: HookType::OnSuccess,
            details: "hook failed".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Hook 'on_success' failed in step 'test_step': hook failed"
        );

        let error = WorkflowError::HookError {
            step_name: StepName::new("test_step"),
            hook_type: HookType::OnFailure,
            details: "cleanup failed".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Hook 'on_failure' failed in step 'test_step': cleanup failed"
        );
    }

    #[test]
    fn test_hook_type_display() {
        assert_eq!(HookType::OnSuccess.to_string(), "on_success");
        assert_eq!(HookType::OnFailure.to_string(), "on_failure");
    }
}
