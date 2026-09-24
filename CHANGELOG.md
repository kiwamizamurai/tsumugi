# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0]

Initial release.

### Added

- `Step<S = Context>` trait: steps operate on a user-defined state type `S`, or on the
  general-purpose `Context`. Optional `retry_policy`, `timeout`, `on_success` and `on_failure`
  methods configure the step.
- `Next` and `StepResult` for choosing the next step.
- `StepError`, convertible from any error type (and strings) so steps can use `?`.
- Closure steps: `FnStep`, `AsyncFnStep`, `WorkflowBuilder::add_fn` and `add_async_fn`.
- `WorkflowBuilder<S>` with per-step `retry`, `timeout` and `no_timeout` overrides. The first
  step added is the start step unless `start_with` is used.
- Declared transitions with `then` and `terminal`, validated at build time (unknown targets,
  unreachable steps, duplicate names) and at runtime (undeclared transitions).
- `BuildError` for invalid definitions and `ExecutionError` (failed step, `ErrorKind` and
  report) for failed runs.
- `Workflow::to_mermaid` for rendering workflows as Mermaid flowcharts.
- `ExecutionReport` describing every run.
- `Context` with typed `Key<T>` keys and `require` for missing-value errors.
- `RetryPolicy` with fixed delay and exponential backoff (custom factor and cap).
- `tsumugi-core` crate with the runtime-independent traits and types.

[Unreleased]: https://github.com/kiwamizamurai/tsumugi/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kiwamizamurai/tsumugi/releases/tag/v0.1.0
