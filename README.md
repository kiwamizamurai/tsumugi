# Tsumugi (紡)

[![Crates.io](https://img.shields.io/crates/v/tsumugi.svg)](https://crates.io/crates/tsumugi)
[![Documentation](https://docs.rs/tsumugi/badge.svg)](https://docs.rs/tsumugi)
[![CI](https://github.com/kiwamizamurai/tsumugi/actions/workflows/ci.yml/badge.svg)](https://github.com/kiwamizamurai/tsumugi/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/tsumugi.svg)](LICENSE)

A lightweight workflow engine for Rust. The name "Tsumugi" (紡) means "to spin" or "to weave" in Japanese.

## Features

- **Type-safe**: `StepName` and `ContextKey` newtypes prevent typos at compile time
- **Async First**: Built with `async-trait` for asynchronous workflows
- **Retry Support**: Fixed delay and exponential backoff policies
- **Configurable Timeouts**: Per-step timeout settings (default: 30s)
- **Error Handling**: Structured errors with `thiserror`, lifecycle hooks
- **Lightweight**: Minimal dependencies

## Installation

```toml
[dependencies]
tsumugi = "0.1"
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use tsumugi::prelude::*;
use async_trait::async_trait;

define_step!(HelloStep);

#[async_trait]
impl Step<String> for HelloStep {
    async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<StepName>, WorkflowError> {
        ctx.insert("message", "Hello, World!".to_string());
        Ok(None)
    }
}

#[tokio::main]
async fn main() {
    let workflow = Workflow::builder()
        .add::<HelloStep>()
        .start_with_type::<HelloStep>()
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    workflow.execute(&mut ctx).await.expect("workflow failed");

    println!("{}", ctx.get("message").unwrap());
}
```

## Examples

See the [examples](examples/) directory:

- [simple_workflow.rs](examples/simple_workflow.rs) - Basic single-step workflow
- [order_workflow.rs](examples/order_workflow.rs) - Multi-step workflow with branching
- [user_scoring_workflow.rs](examples/user_scoring_workflow.rs) - Data processing pipeline

## Documentation

- [API Documentation](https://docs.rs/tsumugi) - Full API reference on docs.rs

## Minimum Supported Rust Version

Rust 1.75.0 or later.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
