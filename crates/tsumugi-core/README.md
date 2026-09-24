# tsumugi-core

Core traits and types for [tsumugi](https://crates.io/crates/tsumugi), a lightweight, embeddable
workflow engine for Rust.

This crate contains the `Step` trait, `Context`, `StepOutput` and error types, without any async
runtime dependency. Depend on it to implement reusable steps in a library; applications should
depend on [`tsumugi`](https://crates.io/crates/tsumugi), which re-exports everything here.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option.
