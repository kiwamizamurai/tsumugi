//! Errors produced when building and running workflows.

use crate::report::ExecutionReport;
use std::error::Error;
use std::fmt;
use tsumugi_core::{Failure, StepError, StepName};

/// An error returned by [`WorkflowBuilder::build`](crate::WorkflowBuilder::build)
/// when the workflow definition is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    /// No steps were added.
    Empty,
    /// The step passed to `start_with` is not registered.
    UnknownStartStep(StepName),
    /// Two steps were registered under the same name.
    DuplicateStep(StepName),
    /// A step declares a transition to a step that is not registered.
    UnknownTransition {
        /// The step declaring the transition.
        from: StepName,
        /// The missing target step.
        to: StepName,
    },
    /// A step can never be reached from the start step.
    ///
    /// Only reported when every step reachable from the start step declares
    /// its transitions, so that reachability can be determined.
    UnreachableStep(StepName),
    /// A step was declared terminal but also has transitions.
    TerminalWithTransitions(StepName),
    /// A step modifier such as `retry` or `then` was called before any step
    /// was added.
    ModifierWithoutStep(&'static str),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Empty => write!(f, "workflow has no steps"),
            BuildError::UnknownStartStep(name) => {
                write!(f, "start step '{}' is not registered", name)
            }
            BuildError::DuplicateStep(name) => write!(f, "step '{}' is registered twice", name),
            BuildError::UnknownTransition { from, to } => {
                write!(
                    f,
                    "step '{}' declares a transition to unknown step '{}'",
                    from, to
                )
            }
            BuildError::UnreachableStep(name) => {
                write!(f, "step '{}' is unreachable from the start step", name)
            }
            BuildError::TerminalWithTransitions(name) => {
                write!(
                    f,
                    "step '{}' is declared terminal but has transitions",
                    name
                )
            }
            BuildError::ModifierWithoutStep(method) => {
                write!(f, "`{}` was called before any step was added", method)
            }
        }
    }
}

impl Error for BuildError {}

/// Why a workflow execution failed. See [`ExecutionError::kind`].
#[derive(Debug)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The step failed after exhausting its retries.
    Step(Failure),
    /// The step's [`on_success`](tsumugi_core::Step::on_success) hook failed.
    Hook(StepError),
    /// The step continued to a step that is not registered.
    UnknownStep(StepName),
    /// The step continued to a step that is not among its declared
    /// transitions (see [`then`](crate::WorkflowBuilder::then)).
    UndeclaredTransition(StepName),
}

/// An error returned by [`Workflow::run`](crate::Workflow::run).
///
/// Every failure is attributed to the step that was running. The error also
/// carries the [`ExecutionReport`] of the steps executed up to that point.
///
/// # Examples
///
/// ```
/// use tsumugi::prelude::*;
/// use tsumugi::{ErrorKind, Failure};
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// let workflow = Workflow::builder()
///     .add_fn("parse", |ctx| {
///         let n: u32 = "not a number".parse()?;
///         ctx.insert("n", n);
///         Ok(Next::Done)
///     })
///     .build()
///     .expect("valid workflow");
///
/// let err = workflow.run(&mut Context::new()).await.unwrap_err();
///
/// assert_eq!(err.step(), "parse");
/// assert_eq!(err.to_string(), "step 'parse' failed: invalid digit found in string");
/// match err.kind() {
///     ErrorKind::Step(Failure::Error(e)) => {
///         assert!(e.downcast_ref::<std::num::ParseIntError>().is_some())
///     }
///     other => panic!("unexpected: {:?}", other),
/// }
/// # }
/// ```
#[derive(Debug)]
pub struct ExecutionError {
    step: StepName,
    kind: ErrorKind,
    report: ExecutionReport,
}

impl ExecutionError {
    pub(crate) fn new(step: StepName, kind: ErrorKind, report: ExecutionReport) -> Self {
        Self { step, kind, report }
    }

    /// Returns the step that failed.
    pub fn step(&self) -> &StepName {
        &self.step
    }

    /// Returns why the workflow failed.
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// Returns the report of the steps executed before the failure,
    /// including the failed step.
    pub fn report(&self) -> &ExecutionReport {
        &self.report
    }

    /// Returns `true` if the step failed because its last attempt timed out.
    pub fn is_timeout(&self) -> bool {
        matches!(self.kind, ErrorKind::Step(Failure::Timeout(_)))
    }

    /// Splits the error into the failed step, the cause and the report.
    pub fn into_parts(self) -> (StepName, ErrorKind, ExecutionReport) {
        (self.step, self.kind, self.report)
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let step = &self.step;
        match &self.kind {
            ErrorKind::Step(Failure::Error(error)) => {
                write!(f, "step '{}' failed: {}", step, error)
            }
            ErrorKind::Step(Failure::Timeout(after)) => {
                write!(f, "step '{}' timed out after {:?}", step, after)
            }
            ErrorKind::Step(failure) => write!(f, "step '{}' failed: {}", step, failure),
            ErrorKind::Hook(error) => {
                write!(f, "on_success hook of step '{}' failed: {}", step, error)
            }
            ErrorKind::UnknownStep(to) => {
                write!(f, "step '{}' continued to unknown step '{}'", step, to)
            }
            ErrorKind::UndeclaredTransition(to) => write!(
                f,
                "step '{}' continued to '{}', which is not a declared transition",
                step, to
            ),
        }
    }
}

impl Error for ExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            ErrorKind::Step(Failure::Error(error)) | ErrorKind::Hook(error) => {
                let error: &(dyn Error + 'static) = error.as_error();
                Some(error)
            }
            _ => None,
        }
    }
}
