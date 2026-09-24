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

    /// Two steps were registered under the same name.
    #[error("Duplicate step name: {0}")]
    DuplicateStep(StepName),

    /// A step declares a transition to a step that is not registered.
    #[error("Step '{from}' declares a transition to unknown step '{to}'")]
    UnknownTransitionTarget {
        /// The step declaring the transition.
        from: StepName,
        /// The missing target step.
        to: StepName,
    },

    /// A step can never be reached from the start step.
    ///
    /// Only reported when every step reachable from the start step declares
    /// its transitions, so that reachability can be determined.
    #[error("Step '{0}' is unreachable from the start step")]
    UnreachableStep(StepName),

    /// A step continued to a step that is not among its declared transitions.
    #[error("Step '{from}' continued to '{to}', which is not a declared transition")]
    UndeclaredTransition {
        /// The step that returned the transition.
        from: StepName,
        /// The requested next step.
        to: StepName,
    },

    /// A lifecycle hook failed.
    ///
    /// Returned when [`Step::on_success`](crate::Step::on_success) fails.
    /// Errors from [`Step::on_failure`](crate::Step::on_failure) are logged
    /// instead, so that the original step error is preserved.
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
