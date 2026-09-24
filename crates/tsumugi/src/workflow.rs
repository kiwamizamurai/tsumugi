//! Workflow definition and execution.

use crate::error::{BuildError, ErrorKind, ExecutionError};
use crate::report::{ExecutionReport, StepRecord, StepStatus};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::time::{Duration, Instant};
use tracing::{info, warn};
use tsumugi_core::{
    AsyncFnStep, BoxFuture, Context, Failure, FnStep, Next, RetryPolicy, Step, StepName, StepResult,
};

/// A validated set of steps and the transitions between them, operating on a
/// state of type `S`.
///
/// Workflows are built with [`Workflow::builder`] (for the default
/// [`Context`] state) or [`WorkflowBuilder::new`] (for a custom state type),
/// and can be run any number of times, concurrently if needed.
///
/// # Examples
///
/// Using a dedicated state type:
///
/// ```
/// use tsumugi::prelude::*;
///
/// #[derive(Default)]
/// struct Signup {
///     email: String,
///     valid: bool,
/// }
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let workflow = WorkflowBuilder::<Signup>::new()
///     .add_fn("validate", |signup| {
///         signup.valid = signup.email.contains('@');
///         Ok(if signup.valid { Next::step("welcome") } else { Next::Done })
///     })
///     .then(["welcome"])
///     .add_fn("welcome", |_signup| Ok(Next::Done))
///     .terminal()
///     .build()?;
///
/// let mut signup = Signup { email: "alice@example.com".into(), ..Default::default() };
/// workflow.run(&mut signup).await?;
/// assert!(signup.valid);
/// # Ok(())
/// # }
/// ```
pub struct Workflow<S = Context> {
    /// Steps in registration order.
    pub(crate) steps: Vec<StepEntry<S>>,
    /// Maps step names to their position in `steps`.
    pub(crate) index: HashMap<StepName, usize>,
    /// Position of the start step in `steps`.
    pub(crate) start: usize,
}

pub(crate) struct StepEntry<S> {
    pub(crate) name: StepName,
    step: Box<dyn Step<S>>,
    timeout: Option<Duration>,
    retry_policy: RetryPolicy,
    /// Declared successor steps. `None` means undeclared (any transition is
    /// allowed); `Some(vec![])` means the step is terminal.
    pub(crate) transitions: Option<Vec<StepName>>,
}

impl<S: Send> fmt::Debug for Workflow<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workflow")
            .field("steps", &self.step_names().collect::<Vec<_>>())
            .field("start_step", self.start_step())
            .finish()
    }
}

impl Workflow {
    /// Creates a builder for a workflow using the default [`Context`] state.
    ///
    /// Use [`WorkflowBuilder::new`] for a custom state type.
    pub fn builder() -> WorkflowBuilder {
        WorkflowBuilder::new()
    }
}

impl<S: Send> Workflow<S> {
    /// Returns the name of the start step.
    pub fn start_step(&self) -> &StepName {
        &self.steps[self.start].name
    }

    /// Returns the names of all steps in registration order.
    pub fn step_names(&self) -> impl Iterator<Item = &StepName> {
        self.steps.iter().map(|entry| &entry.name)
    }

    /// Returns `true` if a step with the given name exists.
    pub fn has_step(&self, name: &str) -> bool {
        self.index.contains_key(name)
    }

    /// Runs the workflow on `state`, starting from the start step.
    ///
    /// On success, returns an [`ExecutionReport`] describing the executed
    /// steps. On failure, returns an [`ExecutionError`] identifying the failed
    /// step, the cause and the report up to that point.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::prelude::*;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let workflow = Workflow::builder()
    ///     .add_fn("hello", |ctx| {
    ///         ctx.insert("message", "hello".to_string());
    ///         Ok(Next::Done)
    ///     })
    ///     .build()?;
    ///
    /// let mut ctx = Context::new();
    /// let report = workflow.run(&mut ctx).await?;
    /// println!("{}", report);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run(&self, state: &mut S) -> Result<ExecutionReport, ExecutionError> {
        let started = Instant::now();
        let mut report = ExecutionReport::default();
        let mut current = self.start;

        loop {
            let entry = &self.steps[current];
            let (record, result) = Self::run_step(entry, state).await;
            report.steps.push(record);
            report.duration = started.elapsed();

            let next = match result {
                Ok(Next::Step(next)) => next,
                Ok(Next::Done) => return Ok(report),
                Err(kind) => return Err(ExecutionError::new(entry.name.clone(), kind, report)),
            };

            if let Some(allowed) = &entry.transitions {
                if !allowed.contains(&next) {
                    let kind = ErrorKind::UndeclaredTransition(next);
                    return Err(ExecutionError::new(entry.name.clone(), kind, report));
                }
            }

            current = match self.index.get(&next) {
                Some(&i) => i,
                None => {
                    let kind = ErrorKind::UnknownStep(next);
                    return Err(ExecutionError::new(entry.name.clone(), kind, report));
                }
            };
        }
    }

    /// Runs a single step with retries and hooks.
    async fn run_step(
        entry: &StepEntry<S>,
        state: &mut S,
    ) -> (StepRecord, Result<Next, ErrorKind>) {
        let started = Instant::now();
        let max_retries = entry.retry_policy.max_retries();
        let mut attempts = 0;

        let outcome = loop {
            attempts += 1;
            let failure = match Self::run_attempt(entry, state).await {
                Ok(next) => break Ok(next),
                Err(failure) => failure,
            };

            let retry = attempts - 1;
            if retry >= max_retries {
                break Err(failure);
            }

            info!(
                "Step '{}' failed ({}), retrying ({}/{})",
                entry.name, failure, attempts, max_retries
            );
            tokio::time::sleep(entry.retry_policy.delay(retry)).await;
        };

        let result = match outcome {
            Ok(next) => match entry.step.on_success(state).await {
                Ok(()) => {
                    info!("Step '{}' completed", entry.name);
                    Ok(next)
                }
                Err(error) => {
                    warn!("Step '{}' on_success hook failed: {}", entry.name, error);
                    Err(ErrorKind::Hook(error))
                }
            },
            Err(failure) => {
                warn!(
                    "Step '{}' failed after {} attempt(s): {}",
                    entry.name, attempts, failure
                );
                entry.step.on_failure(state, &failure).await;
                Err(ErrorKind::Step(failure))
            }
        };

        let status = match &result {
            Ok(Next::Step(next)) => StepStatus::Continued(next.clone()),
            Ok(Next::Done) => StepStatus::Completed,
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
    async fn run_attempt(entry: &StepEntry<S>, state: &mut S) -> Result<Next, Failure> {
        let result: StepResult = match entry.timeout {
            Some(limit) => match tokio::time::timeout(limit, entry.step.run(state)).await {
                Ok(result) => result,
                Err(_) => return Err(Failure::Timeout(limit)),
            },
            None => entry.step.run(state).await,
        };
        result.map_err(Failure::Error)
    }

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

/// Builder for a [`Workflow`] operating on a state of type `S`.
///
/// Steps are added with [`add_step`](Self::add_step), [`add_fn`](Self::add_fn)
/// and [`add_async_fn`](Self::add_async_fn). The modifiers
/// [`retry`](Self::retry), [`timeout`](Self::timeout),
/// [`no_timeout`](Self::no_timeout), [`then`](Self::then) and
/// [`terminal`](Self::terminal) configure the most recently added step:
///
/// ```
/// use std::time::Duration;
/// use tsumugi::prelude::*;
///
/// let workflow = Workflow::builder()
///     .add_fn("fetch", |_ctx| Ok(Next::step("save")))
///     .retry(RetryPolicy::exponential(3, Duration::from_millis(100)))
///     .timeout(Duration::from_secs(5))
///     .then(["save"])
///     .add_fn("save", |_ctx| Ok(Next::Done))
///     .terminal()
///     .build();
///
/// assert!(workflow.is_ok());
/// ```
///
/// The first step added is the start step unless
/// [`start_with`](Self::start_with) says otherwise. Configuration mistakes are
/// reported by [`build`](Self::build).
pub struct WorkflowBuilder<S = Context> {
    steps: Vec<StepEntry<S>>,
    start_step: Option<StepName>,
    errors: Vec<BuildError>,
}

impl<S: Send> Default for WorkflowBuilder<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Send> WorkflowBuilder<S> {
    /// Creates an empty builder.
    ///
    /// For the default [`Context`] state, [`Workflow::builder`] is shorter.
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            start_step: None,
            errors: Vec::new(),
        }
    }

    /// Adds a step under the given name.
    ///
    /// The step's own [`retry_policy`](Step::retry_policy) and
    /// [`timeout`](Step::timeout) apply unless overridden with
    /// [`retry`](Self::retry), [`timeout`](Self::timeout) or
    /// [`no_timeout`](Self::no_timeout).
    pub fn add_step(mut self, name: impl Into<StepName>, step: impl Step<S> + 'static) -> Self {
        let name = name.into();
        if self.steps.iter().any(|entry| entry.name == name) {
            self.errors.push(BuildError::DuplicateStep(name));
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

    /// Adds a step backed by a synchronous closure. See [`FnStep`].
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::prelude::*;
    ///
    /// let workflow = Workflow::builder()
    ///     .add_fn("check", |ctx| {
    ///         let age = *ctx.require::<u32>("age")?;
    ///         Ok(Next::step(if age >= 18 { "adult" } else { "minor" }))
    ///     })
    ///     .add_fn("adult", |_ctx| Ok(Next::Done))
    ///     .add_fn("minor", |_ctx| Ok(Next::Done))
    ///     .build();
    ///
    /// assert!(workflow.is_ok());
    /// ```
    pub fn add_fn<F>(self, name: impl Into<StepName>, func: F) -> Self
    where
        F: Fn(&mut S) -> StepResult + Send + Sync + 'static,
    {
        self.add_step(name, FnStep::new(func))
    }

    /// Adds a step backed by an asynchronous closure. See [`AsyncFnStep`].
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::prelude::*;
    ///
    /// let workflow = Workflow::builder()
    ///     .add_async_fn("fetch", |ctx| {
    ///         Box::pin(async move {
    ///             ctx.insert("body", "fetched".to_string());
    ///             Ok(Next::Done)
    ///         })
    ///     })
    ///     .build();
    ///
    /// assert!(workflow.is_ok());
    /// ```
    pub fn add_async_fn<F>(self, name: impl Into<StepName>, func: F) -> Self
    where
        F: for<'a> Fn(&'a mut S) -> BoxFuture<'a, StepResult> + Send + Sync + 'static,
    {
        self.add_step(name, AsyncFnStep::new(func))
    }

    /// Returns the most recently added step, recording an error if there is
    /// none.
    fn last_step(&mut self, method: &'static str) -> Option<&mut StepEntry<S>> {
        if self.steps.is_empty() {
            self.errors.push(BuildError::ModifierWithoutStep(method));
        }
        self.steps.last_mut()
    }

    /// Sets the retry policy of the most recently added step, overriding
    /// [`Step::retry_policy`].
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
    ///   ([`ErrorKind::UndeclaredTransition`]),
    /// - edges in the [Mermaid diagram](Workflow::to_mermaid).
    ///
    /// Completing the workflow with [`Next::Done`] is always allowed. Use
    /// [`terminal`](Self::terminal) for steps that never continue. Calling
    /// `then` again on the same step adds more targets.
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
    /// part in build-time validation, and continuing from it at runtime fails
    /// with [`ErrorKind::UndeclaredTransition`].
    pub fn terminal(mut self) -> Self {
        let mut conflict = None;
        if let Some(entry) = self.last_step("terminal") {
            match &entry.transitions {
                Some(targets) if !targets.is_empty() => conflict = Some(entry.name.clone()),
                _ => entry.transitions = Some(Vec::new()),
            }
        }
        if let Some(name) = conflict {
            self.errors.push(BuildError::TerminalWithTransitions(name));
        }
        self
    }

    /// Sets the start step. Defaults to the first step added.
    pub fn start_with(mut self, name: impl Into<StepName>) -> Self {
        self.start_step = Some(name.into());
        self
    }

    /// Validates the definition and builds the workflow.
    ///
    /// # Errors
    ///
    /// Returns the first problem found; see [`BuildError`].
    pub fn build(self) -> Result<Workflow<S>, BuildError> {
        if let Some(error) = self.errors.into_iter().next() {
            return Err(error);
        }

        let index: HashMap<StepName, usize> = self
            .steps
            .iter()
            .enumerate()
            .map(|(i, entry)| (entry.name.clone(), i))
            .collect();

        let start = match self.start_step {
            Some(name) => *index.get(&name).ok_or(BuildError::UnknownStartStep(name))?,
            None if self.steps.is_empty() => return Err(BuildError::Empty),
            None => 0,
        };

        for entry in &self.steps {
            for target in entry.transitions.iter().flatten() {
                if !index.contains_key(target) {
                    return Err(BuildError::UnknownTransition {
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
            return Err(BuildError::UnreachableStep(unreachable.clone()));
        }

        Ok(workflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsumugi_core::async_trait;

    struct SuccessStep;

    #[async_trait]
    impl Step for SuccessStep {
        async fn run(&self, ctx: &mut Context) -> StepResult {
            ctx.insert("success", true);
            Ok(Next::Done)
        }
    }

    struct FailureStep;

    #[async_trait]
    impl Step for FailureStep {
        async fn run(&self, _ctx: &mut Context) -> StepResult {
            Err("intentional failure".into())
        }
    }

    fn done(_ctx: &mut Context) -> StepResult {
        Ok(Next::Done)
    }

    #[tokio::test]
    async fn test_workflow_success() {
        let workflow = Workflow::builder()
            .add_step("success", SuccessStep)
            .build()
            .expect("valid workflow");

        let mut ctx = Context::new();
        let report = workflow.run(&mut ctx).await.expect("workflow succeeds");
        assert_eq!(ctx.get::<bool>("success"), Some(&true));
        assert_eq!(report.steps().len(), 1);
        assert_eq!(report.steps()[0].status(), &StepStatus::Completed);
    }

    #[tokio::test]
    async fn test_workflow_failure() {
        let workflow = Workflow::builder()
            .add_step("failure", FailureStep)
            .build()
            .expect("valid workflow");

        let err = workflow.run(&mut Context::new()).await.unwrap_err();
        assert_eq!(err.step(), "failure");
        assert!(matches!(err.kind(), ErrorKind::Step(Failure::Error(_))));
        assert_eq!(
            err.to_string(),
            "step 'failure' failed: intentional failure"
        );
        assert_eq!(err.report().steps()[0].status(), &StepStatus::Failed);
    }

    #[test]
    fn test_empty_workflow_is_rejected() {
        let result = Workflow::builder().build();
        assert!(matches!(result, Err(BuildError::Empty)));
    }

    #[test]
    fn test_start_defaults_to_first_step() {
        let workflow = Workflow::builder()
            .add_fn("first", done)
            .add_fn("second", done)
            .build()
            .expect("valid workflow");
        assert_eq!(workflow.start_step(), "first");
    }

    #[test]
    fn test_unknown_start_step_is_rejected() {
        let result = Workflow::builder()
            .add_fn("a", done)
            .start_with("missing")
            .build();
        assert!(matches!(result, Err(BuildError::UnknownStartStep(name)) if name == "missing"));
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
            .build();
        assert!(matches!(result, Err(BuildError::DuplicateStep(name)) if name == "a"));
    }

    #[test]
    fn test_modifier_before_any_step_is_rejected() {
        let result = Workflow::builder().then(["a"]).add_fn("a", done).build();
        assert_eq!(result.unwrap_err(), BuildError::ModifierWithoutStep("then"));
    }

    #[test]
    fn test_terminal_with_transitions_is_rejected() {
        let result = Workflow::builder()
            .add_fn("a", done)
            .then(["a"])
            .terminal()
            .build();
        assert!(matches!(result, Err(BuildError::TerminalWithTransitions(name)) if name == "a"));
    }

    #[test]
    fn test_unknown_transition_is_rejected() {
        let result = Workflow::builder()
            .add_fn("a", done)
            .then(["missing"])
            .build();
        assert!(matches!(
            result,
            Err(BuildError::UnknownTransition { from, to }) if from == "a" && to == "missing"
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
            .build();
        assert!(matches!(result, Err(BuildError::UnreachableStep(name)) if name == "orphan"));
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
            .build();
        assert!(result.is_ok());
    }
}
