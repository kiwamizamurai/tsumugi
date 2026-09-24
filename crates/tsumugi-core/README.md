# tsumugi-core

Core traits and types for [tsumugi](https://crates.io/crates/tsumugi), a lightweight, embeddable
workflow engine for Rust.

This crate contains the `Step` trait, `Next`, `StepError`, `RetryPolicy` and `Context`, without
any async runtime dependency. Depend on it to implement reusable steps in a library; applications
should depend on [`tsumugi`](https://crates.io/crates/tsumugi), which re-exports everything here.

## License

Licensed under the [MIT license](LICENSE).
