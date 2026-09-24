//! Workflow engine for executing steps.

use crate::report::{ExecutionError, ExecutionReport, StepRecord, StepStatus};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::time::{Duration, Instant};
use tracing::{info, warn};
use tsumugi_core::{
    AsyncFnStep, BoxFuture, Context, FnStep, HookType, RetryPolicy, Step, StepName, StepOutput,
    WorkflowError,
};

/// A workflow engine that executes a series of steps.
///
/// Steps are identified by the name they are registered under. Execution
/// starts at the start step and follows the [`StepOutput`] returned by each
/// step until one completes the workflow or fails.
pub struct Workflow {
    /// Steps in registration order.
    pub(crate) steps: Vec<StepEntry>,
    /// Maps step names to their position in `steps`.
    pub(crate) index: HashMap<StepName, usize>,
    /// Position of the start step in `steps`.
    pub(crate) start: usize,
}

pub(crate) struct StepEntry {
    pub(crate) name: StepName,
    step: Box<dyn Step>,
    timeout: Option<Duration>,
    retry_policy: RetryPolicy,
    /// Declared successor steps. `None` means undeclared (any transition is
    /// allowed); `Some(vec![])` means the step is terminal.
    pub(crate) transitions: Option<Vec<StepName>>,
}

impl fmt::Debug for Workflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workflow")
            .field("steps", &self.step_names().collect::<Vec<_>>())
            .field("start_step", self.start_step())
            .finish()
    }
}

impl Workflow {
    /// Creates a new workflow builder.
    pub fn builder() -> WorkflowBuilder {
        WorkflowBuilder::new()
    }

    /// Returns the name of the start step.
    pub fn start_step(&self) -> &StepName {
        &self.steps[self.start].name
    }

    /// Returns an iterator over all registered step names in registration order.
    pub fn step_names(&self) -> impl Iterator<Item = &StepName> {
        self.steps.iter().map(|entry| &entry.name)
    }

    /// Returns `true` if a step with the given name exists.
    pub fn has_step(&self, name: &str) -> bool {
        self.index.contains_key(name)
    }

    /// Returns the number of registered steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Executes the workflow starting from the configured start step.
    ///
    /// On success, returns an [`ExecutionReport`] describing the executed
    /// steps. On failure, returns an [`ExecutionError`] holding the error that
    /// stopped the workflow and the report up to that point.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::prelude::*;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// let workflow = Workflow::builder()
    ///     .add_fn("hello", |ctx| {
    ///         ctx.insert("message", "hello".to_string());
    ///         Ok(StepOutput::done())
    ///     })
    ///     .start_with("hello")
    ///     .build()
    ///     .expect("valid workflow");
    ///
    /// let mut ctx = Context::new();
    /// match workflow.execute(&mut ctx).await {
    ///     Ok(report) => println!("{}", report),
    ///     Err(err) => eprintln!("failed: {}\n{}", err, err.report()),
    /// }
    /// # }
    /// ```
    pub async fn execute(&self, ctx: &mut Context) -> Result<ExecutionReport, ExecutionError> {
        let started = Instant::now();
        let mut report = ExecutionReport::default();
        let mut current = self.start;

        loop {
            let entry = &self.steps[current];
            let (record, result) = Self::execute_step(entry, ctx).await;
            report.steps.push(record);
            report.duration = started.elapsed();

            let next = match result {
                Ok(StepOutput::Continue(next)) => next,
                Ok(StepOutput::Complete) => return Ok(report),
                Err(error) => return Err(ExecutionError::new(error, report)),
            };

            if let Some(allowed) = &entry.transitions {
                if !allowed.contains(&next) {
                    let error = WorkflowError::UndeclaredTransition {
                        from: entry.name.clone(),
                        to: next,
                    };
                    return Err(ExecutionError::new(error, report));
                }
            }

            current = match self.index.get(&next) {
                Some(&i) => i,
                None => {
                    return Err(ExecutionError::new(
                        WorkflowError::StepNotFound(next),
                        report,
                    ))
                }
            };
        }
    }

    /// Executes a single step, retrying according to its policy, and runs its
    /// lifecycle hooks.
    async fn execute_step(
        entry: &StepEntry,
        ctx: &mut Context,
    ) -> (StepRecord, Result<StepOutput, WorkflowError>) {
        let started = Instant::now();
        let max_retries = entry.retry_policy.max_retries();
        let mut attempts = 0;

        let result = loop {
            attempts += 1;
            let error = match Self::execute_attempt(entry, ctx).await {
                Ok(output) => break Ok(output),
                Err(error) => error,
            };

            let retries = attempts - 1;
            if retries >= max_retries {
                break Err(error);
            }

            info!(
                "Step '{}' failed ({}), retrying ({}/{})",
                entry.name, error, attempts, max_retries
            );
            if let Some(delay) = entry.retry_policy.delay_for_attempt(retries) {
                tokio::time::sleep(delay).await;
            }
        };

        let result = match result {
            Ok(output) => match entry.step.on_success(ctx).await {
                Ok(()) => {
                    info!("Step '{}' completed successfully", entry.name);
                    Ok(output)
                }
                Err(hook_error) => {
                    warn!(
                        "Step '{}' on_success hook failed: {}",
                        entry.name, hook_error
                    );
                    Err(WorkflowError::HookError {
                        step_name: entry.name.clone(),
                        hook_type: HookType::OnSuccess,
                        details: hook_error.to_string(),
                    })
                }
            },
            Err(error) => {
                warn!(
                    "Step '{}' failed after {} attempt(s): {}",
                    entry.name, attempts, error
                );
                if let Err(hook_error) = entry.step.on_failure(ctx, &error).await {
                    warn!(
                        "Step '{}' on_failure hook failed: {}",
                        entry.name, hook_error
                    );
                }
                Err(error)
            }
        };

        let status = match &result {
            Ok(StepOutput::Continue(next)) => StepStatus::Continued(next.clone()),
            Ok(StepOutput::Complete) => StepStatus::Completed,
            Err(_) => StepStatus::Failed,
        };
        let record = StepRecord {
            name: entry.name.clone(),
            attempts,
            duration: started.elapsed(),
            status,
        };
        (record, result)
    }

    /// Runs one attempt of a step, applying its timeout if any.
    async fn execute_attempt(
        entry: &StepEntry,
        ctx: &mut Context,
    ) -> Result<StepOutput, WorkflowError> {
        match entry.timeout {
            Some(limit) => tokio::time::timeout(limit, entry.step.execute(ctx))
                .await
                .unwrap_or_else(|_| {
                    Err(WorkflowError::Timeout {
                        step_name: entry.name.clone(),
                    })
                }),
            None => entry.step.execute(ctx).await,
        }
    }
}

/// Builder for constructing [`Workflow`] instances.
///
/// Configuration mistakes (such as calling [`then`](Self::then) before adding
/// a step) are collected and reported by [`build`](Self::build).
#[derive(Default)]
pub struct WorkflowBuilder {
    steps: Vec<StepEntry>,
    start_step: Option<StepName>,
    errors: Vec<WorkflowError>,
}

impl WorkflowBuilder {
    /// Creates a new empty workflow builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the most recently added step, recording a configuration error
    /// if there is none.
    fn last_step(&mut self, method: &str) -> Option<&mut StepEntry> {
        if self.steps.is_empty() {
            self.errors.push(WorkflowError::Configuration(format!(
                "`{}` must be called after adding a step",
                method
            )));
        }
        self.steps.last_mut()
    }

    /// Adds a step under the given name.
    ///
    /// The step's own [`retry_policy`](Step::retry_policy) and
    /// [`timeout`](Step::timeout) are used unless overridden with
    /// [`retry`](Self::retry), [`timeout`](Self::timeout) or
    /// [`no_timeout`](Self::no_timeout).
    ///
    /// The name identifies the step within the workflow: it is the target of
    /// [`StepOutput::next`] and appears in logs, errors and reports.
    pub fn add_step<S: Step + 'static>(mut self, name: impl Into<StepName>, step: S) -> Self {
        let name = name.into();
        if self.steps.iter().any(|entry| entry.name == name) {
            self.errors.push(WorkflowError::DuplicateStep(name));
            return self;
        }
        self.steps.push(StepEntry {
            name,
            timeout: step.timeout(),
            retry_policy: step.retry_policy(),
            step: Box::new(step),
            transitions: None,
        });
        self
    }

    /// Adds a step backed by a synchronous closure.
    ///
    /// Shorthand for `add_step(name, FnStep::new(name, func))`. See [`FnStep`]
    /// for details.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::prelude::*;
    ///
    /// let workflow = Workflow::builder()
    ///     .add_fn("check", |ctx| {
    ///         let age = ctx.get::<u32>("age").copied().unwrap_or_default();
    ///         Ok(StepOutput::next(if age >= 18 { "adult" } else { "minor" }))
    ///     })
    ///     .add_fn("adult", |_ctx| Ok(StepOutput::done()))
    ///     .add_fn("minor", |_ctx| Ok(StepOutput::done()))
    ///     .start_with("check")
    ///     .build();
    ///
    /// assert!(workflow.is_ok());
    /// ```
    pub fn add_fn<F>(self, name: impl Into<StepName>, func: F) -> Self
    where
        F: Fn(&mut Context) -> Result<StepOutput, WorkflowError> + Send + Sync + 'static,
    {
        let step_name = name.into();
        let step = FnStep::new(step_name.clone(), func);
        self.add_step(step_name, step)
    }

    /// Adds a step backed by an asynchronous closure.
    ///
    /// Shorthand for `add_step(name, AsyncFnStep::new(name, func))`. See
    /// [`AsyncFnStep`] for details.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use tsumugi::prelude::*;
    ///
    /// let workflow = Workflow::builder()
    ///     .add_async_fn("fetch", |ctx| {
    ///         Box::pin(async move {
    ///             ctx.insert("body", "fetched".to_string());
    ///             Ok(StepOutput::done())
    ///         })
    ///     })
    ///     .retry(RetryPolicy::fixed(3, Duration::from_millis(100)))
    ///     .timeout(Duration::from_secs(5))
    ///     .start_with("fetch")
    ///     .build();
    ///
    /// assert!(workflow.is_ok());
    /// ```
    pub fn add_async_fn<F>(self, name: impl Into<StepName>, func: F) -> Self
    where
        F: for<'a> Fn(&'a mut Context) -> BoxFuture<'a, Result<StepOutput, WorkflowError>>
            + Send
            + Sync
            + 'static,
    {
        let step_name = name.into();
        let step = AsyncFnStep::new(step_name.clone(), func);
        self.add_step(step_name, step)
    }

    /// Sets the retry policy of the most recently added step, overriding
    /// [`Step::retry_policy`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use tsumugi::prelude::*;
    ///
    /// let workflow = Workflow::builder()
    ///     .add_fn("flaky", |_ctx| Ok(StepOutput::done()))
    ///     .retry(RetryPolicy::exponential(5, Duration::from_millis(100)))
    ///     .start_with("flaky")
    ///     .build();
    ///
    /// assert!(workflow.is_ok());
    /// ```
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        if let Some(entry) = self.last_step("retry") {
            entry.retry_policy = policy;
        }
        self
    }

    /// Sets the per-attempt timeout of the most recently added step,
    /// overriding [`Step::timeout`].
    pub fn timeout(mut self, duration: Duration) -> Self {
        if let Some(entry) = self.last_step("timeout") {
            entry.timeout = Some(duration);
        }
        self
    }

    /// Disables the timeout of the most recently added step, overriding
    /// [`Step::timeout`].
    pub fn no_timeout(mut self) -> Self {
        if let Some(entry) = self.last_step("no_timeout") {
            entry.timeout = None;
        }
        self
    }

    /// Declares the steps the most recently added step may continue to.
    ///
    /// Declaring transitions is optional, but enables:
    ///
    /// - validation at [`build`](Self::build) time that every target exists,
    /// - detection of unreachable steps (when all reachable steps declare
    ///   their transitions),
    /// - a runtime check that the step only continues to a declared target
    ///   ([`WorkflowError::UndeclaredTransition`]),
    /// - edges in the [Mermaid diagram](Workflow::to_mermaid).
    ///
    /// Completing the workflow with [`StepOutput::done`] is always allowed.
    /// Use [`terminal`](Self::terminal) for steps that never continue.
    /// Calling `then` again on the same step adds more targets.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::prelude::*;
    ///
    /// let workflow = Workflow::builder()
    ///     .add_fn("validate", |_ctx| Ok(StepOutput::next("save")))
    ///     .then(["save", "reject"])
    ///     .add_fn("save", |_ctx| Ok(StepOutput::done()))
    ///     .terminal()
    ///     .add_fn("reject", |_ctx| Ok(StepOutput::done()))
    ///     .terminal()
    ///     .start_with("validate")
    ///     .build();
    ///
    /// assert!(workflow.is_ok());
    /// ```
    pub fn then<I>(mut self, targets: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<StepName>,
    {
        if let Some(entry) = self.last_step("then") {
            entry
                .transitions
                .get_or_insert_with(Vec::new)
                .extend(targets.into_iter().map(Into::into));
        }
        self
    }

    /// Declares that the most recently added step never continues to another
    /// step, i.e. it always completes or fails the workflow.
    ///
    /// The step then counts as having declared transitions (none), so it takes
    /// part in build-time validation and continuing from it at runtime fails
    /// with [`WorkflowError::UndeclaredTransition`].
    pub fn terminal(mut self) -> Self {
        let mut conflict = None;
        if let Some(entry) = self.last_step("terminal") {
            match &entry.transitions {
                Some(targets) if !targets.is_empty() => conflict = Some(entry.name.clone()),
                _ => entry.transitions = Some(Vec::new()),
            }
        }
        if let Some(name) = conflict {
            self.errors.push(WorkflowError::Configuration(format!(
                "step '{}' is declared terminal but has transitions",
                name
            )));
        }
        self
    }

    /// Sets the start step by name.
    pub fn start_with(mut self, step_name: impl Into<StepName>) -> Self {
        self.start_step = Some(step_name.into());
        self
    }

    /// Builds the workflow.
    ///
    /// # Errors
    ///
    /// Returns the first configuration problem found:
    ///
    /// - [`WorkflowError::Configuration`] if no start step was set or a
    ///   builder method was misused,
    /// - [`WorkflowError::DuplicateStep`] if a name was registered twice,
    /// - [`WorkflowError::StepNotFound`] if the start step does not exist,
    /// - [`WorkflowError::UnknownTransitionTarget`] if a declared transition
    ///   points to a step that does not exist,
    /// - [`WorkflowError::UnreachableStep`] if a step can never be reached.
    pub fn build(self) -> Result<Workflow, WorkflowError> {
        if let Some(error) = self.errors.into_iter().next() {
            return Err(error);
        }

        let start_step = self.start_step.ok_or_else(|| {
            WorkflowError::Configuration("Start step must be specified".to_string())
        })?;

        let index: HashMap<StepName, usize> = self
            .steps
            .iter()
            .enumerate()
            .map(|(i, entry)| (entry.name.clone(), i))
            .collect();

        let start = *index
            .get(&start_step)
            .ok_or(WorkflowError::StepNotFound(start_step))?;

        for entry in &self.steps {
            for target in entry.transitions.iter().flatten() {
                if !index.contains_key(target) {
                    return Err(WorkflowError::UnknownTransitionTarget {
                        from: entry.name.clone(),
                        to: target.clone(),
                    });
                }
            }
        }

        let workflow = Workflow {
            steps: self.steps,
            index,
            start,
        };

        if let Some(unreachable) = workflow.find_unreachable_step() {
            return Err(WorkflowError::UnreachableStep(unreachable.clone()));
        }

        Ok(workflow)
    }
}

impl Workflow {
    /// Returns the first step (in registration order) that cannot be reached
    /// from the start step.
    ///
    /// Returns `None` if every step is reachable, or if reachability cannot be
    /// determined because a reachable step has undeclared transitions.
    fn find_unreachable_step(&self) -> Option<&StepName> {
        let mut visited = HashSet::from([self.start]);
        let mut queue = VecDeque::from([self.start]);

        while let Some(i) = queue.pop_front() {
            // An undeclared step may continue anywhere.
            let targets = self.steps[i].transitions.as_ref()?;
            for target in targets {
                let j = self.index[target];
                if visited.insert(j) {
                    queue.push_back(j);
                }
            }
        }

        self.steps
            .iter()
            .enumerate()
            .find(|(i, _)| !visited.contains(i))
            .map(|(_, entry)| &entry.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsumugi_core::async_trait;

    #[derive(Debug)]
    struct SuccessStep;

    #[async_trait]
    impl Step for SuccessStep {
        async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
            ctx.insert("success", true);
            Ok(StepOutput::done())
        }
    }

    #[derive(Debug)]
    struct FailureStep;

    #[async_trait]
    impl Step for FailureStep {
        async fn execute(&self, _ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
            Err(WorkflowError::StepError {
                step_name: StepName::new("failure"),
                details: "Intentional failure".to_string(),
            })
        }
    }

    fn done(_ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        Ok(StepOutput::done())
    }

    #[tokio::test]
    async fn test_workflow_success() {
        let workflow = Workflow::builder()
            .add_step("success", SuccessStep)
            .start_with("success")
            .build()
            .expect("valid workflow");

        let mut ctx = Context::new();
        let report = workflow.execute(&mut ctx).await.expect("workflow succeeds");
        assert_eq!(ctx.get::<bool>("success"), Some(&true));
        assert_eq!(report.steps().len(), 1);
        assert_eq!(report.steps()[0].status(), &StepStatus::Completed);
    }

    #[tokio::test]
    async fn test_workflow_failure() {
        let workflow = Workflow::builder()
            .add_step("failure", FailureStep)
            .start_with("failure")
            .build()
            .expect("valid workflow");

        let mut ctx = Context::new();
        let err = workflow.execute(&mut ctx).await.unwrap_err();
        assert!(matches!(err.error(), WorkflowError::StepError { .. }));
        assert_eq!(err.report().steps()[0].status(), &StepStatus::Failed);
    }

    #[test]
    fn test_builder_requires_start_step() {
        let result = Workflow::builder().add_step("step", SuccessStep).build();
        assert!(matches!(result, Err(WorkflowError::Configuration(_))));
    }

    #[test]
    fn test_step_names_keep_registration_order() {
        let workflow = Workflow::builder()
            .add_fn("c", done)
            .add_fn("a", done)
            .add_fn("b", done)
            .start_with("a")
            .build()
            .expect("valid workflow");

        let names: Vec<&str> = workflow.step_names().map(StepName::as_str).collect();
        assert_eq!(names, ["c", "a", "b"]);
    }

    #[test]
    fn test_duplicate_step_is_rejected() {
        let result = Workflow::builder()
            .add_fn("a", done)
            .add_fn("a", done)
            .start_with("a")
            .build();
        assert!(matches!(result, Err(WorkflowError::DuplicateStep(name)) if name.as_str() == "a"));
    }

    #[test]
    fn test_then_before_any_step_is_rejected() {
        let result = Workflow::builder()
            .then(["a"])
            .add_fn("a", done)
            .start_with("a")
            .build();
        assert!(matches!(result, Err(WorkflowError::Configuration(_))));
    }

    #[test]
    fn test_terminal_with_transitions_is_rejected() {
        let result = Workflow::builder()
            .add_fn("a", done)
            .then(["a"])
            .terminal()
            .start_with("a")
            .build();
        assert!(matches!(result, Err(WorkflowError::Configuration(_))));
    }

    #[test]
    fn test_unknown_transition_target_is_rejected() {
        let result = Workflow::builder()
            .add_fn("a", done)
            .then(["missing"])
            .start_with("a")
            .build();
        assert!(matches!(
            result,
            Err(WorkflowError::UnknownTransitionTarget { from, to })
                if from.as_str() == "a" && to.as_str() == "missing"
        ));
    }

    #[test]
    fn test_unreachable_step_is_rejected_when_fully_declared() {
        let result = Workflow::builder()
            .add_fn("a", done)
            .then(["b"])
            .add_fn("b", done)
            .terminal()
            .add_fn("orphan", done)
            .terminal()
            .start_with("a")
            .build();
        assert!(matches!(
            result,
            Err(WorkflowError::UnreachableStep(name)) if name.as_str() == "orphan"
        ));
    }

    #[test]
    fn test_reachability_is_skipped_with_undeclared_steps() {
        // "b" is undeclared, so it may continue to "orphan" at runtime.
        let result = Workflow::builder()
            .add_fn("a", done)
            .then(["b"])
            .add_fn("b", done)
            .add_fn("orphan", done)
            .terminal()
            .start_with("a")
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_cycles_are_allowed() {
        let result = Workflow::builder()
            .add_fn("poll", done)
            .then(["poll", "finish"])
            .add_fn("finish", done)
            .terminal()
            .start_with("poll")
            .build();
        assert!(result.is_ok());
    }
}
