# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0]

Initial release.

### Added

- `Step` trait with optional `retry_policy`, `timeout`, `on_success` and `on_failure` methods.
- Closure-based steps: `FnStep`, `AsyncFnStep`, `WorkflowBuilder::add_fn` and `add_async_fn`.
- `WorkflowBuilder` with per-step `retry`, `timeout` and `no_timeout` overrides.
- Declared transitions with `then` and `terminal`, validated at build time (unknown targets,
  unreachable steps, duplicate names) and at runtime (undeclared transitions).
- `Workflow::to_mermaid` for rendering workflows as Mermaid flowcharts.
- `ExecutionReport` returned by `Workflow::execute`, and `ExecutionError` carrying the report
  on failure.
- `Context` for heterogeneous values, with typed `Key<T>` keys.
- Retry policies: none, fixed delay and exponential backoff.
- `tsumugi-core` crate with the runtime-independent traits and types.

[Unreleased]: https://github.com/kiwamizamurai/tsumugi/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kiwamizamurai/tsumugi/releases/tag/v0.1.0
