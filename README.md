# Tsumugi (紡)

[![Crates.io](https://img.shields.io/crates/v/tsumugi.svg)](https://crates.io/crates/tsumugi)
[![Documentation](https://docs.rs/tsumugi/badge.svg)](https://docs.rs/tsumugi)
[![CI](https://github.com/kiwamizamurai/tsumugi/actions/workflows/ci.yml/badge.svg)](https://github.com/kiwamizamurai/tsumugi/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/tsumugi.svg)](LICENSE)

A lightweight workflow engine for Rust. The name "Tsumugi" (紡) means "to spin" or "to weave" in Japanese.

## Why tsumugi?

Most workflow engines (Airflow, Prefect, Temporal) are **services** that require external databases, message queues, or server processes.

tsumugi is different. It's a **library** you embed directly in your Rust application:

| | tsumugi | Airflow | Prefect | Dagster | Temporal | Argo Workflows |
|--|---------|---------|---------|---------|----------|----------------|
| Type | Library | Platform | Framework | Platform | Platform | K8s CRD |
| Language | Rust | Python | Python | Python | Go + SDKs | Go (YAML) |
| DB required | No | Yes | No | No | Yes | No |
| Server required | No | Yes | No | No | Yes | Yes (K8s) |
| UI | No | Yes | Optional | Yes | Yes | Yes |

## Features

- **Lightweight**: Minimal dependencies, fast compilation, ~1MB binary
- **Zero Infrastructure**: No database, no message queue, no server process
- **Heterogeneous Context**: Store any type directly without wrapper enums
- **Retry & Timeout**: Built-in exponential backoff and per-step timeouts

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

#[derive(Debug)]
struct HelloStep;

#[async_trait]
impl Step for HelloStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        ctx.insert("message", "Hello, World!".to_string());
        Ok(StepOutput::done())
    }

    fn name(&self) -> StepName {
        StepName::new("HelloStep")
    }
}

#[tokio::main]
async fn main() {
    let workflow = Workflow::builder()
        .add_step("hello", HelloStep)
        .start_with("hello")
        .build()
        .expect("valid workflow");

    let mut ctx = Context::new();
    workflow.execute(&mut ctx).await.expect("workflow failed");

    // Retrieve typed data from context
    let message: &String = ctx.get("message").unwrap();
    println!("{}", message);
}
```

## Heterogeneous Context

The context can store any type that implements `Send + Sync + 'static`:

```rust
// Store different types directly - no wrapper enum needed!
ctx.insert("user_id", 123u64);
ctx.insert("name", "Alice".to_string());
ctx.insert("scores", vec![85.5, 92.0, 78.3]);
ctx.insert("config", MyCustomConfig { ... });

// Retrieve with type inference
let id: &u64 = ctx.get("user_id").unwrap();
let name: &String = ctx.get("name").unwrap();
```

## Step Output

Steps return `StepOutput` to control workflow flow:

```rust
async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
    // Continue to next step
    Ok(StepOutput::next("next_step"))

    // Or complete the workflow
    Ok(StepOutput::done())
}
```

## Optional Traits

Extend step behavior with optional traits:

```rust
// Retry support
impl Retryable for MyStep {
    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::exponential_backoff(
            3,                              // max retries
            Duration::from_millis(100),     // initial delay
            Duration::from_secs(5),         // max delay
            2,                              // multiplier
        ).unwrap_or(RetryPolicy::None)
    }
}

// Lifecycle hooks
#[async_trait]
impl WithHooks for MyStep {
    async fn on_success(&self, ctx: &mut Context) -> Result<(), WorkflowError> {
        println!("Step completed!");
        Ok(())
    }

    async fn on_failure(&self, ctx: &mut Context, error: &WorkflowError) -> Result<(), WorkflowError> {
        eprintln!("Step failed: {:?}", error);
        Ok(())
    }
}

// Custom timeout
impl WithTimeout for MyStep {
    fn timeout(&self) -> Duration {
        Duration::from_secs(60)
    }
}
```

## Use Cases

Tsumugi is ideal for lightweight, embeddable workflow automation:

| Use Case | Example |
|----------|---------|
| **ETL Pipelines** | Fetch REST API data, transform, export to CSV |
| **Health Monitoring** | Check multiple endpoints, aggregate status, alert |
| **File Processing** | Batch transform logs, convert formats |
| **Data Validation** | Multi-stage validation for CI/CD gates |
| **Notifications** | Multi-channel dispatch (Email, Slack, Webhook) |
| **GitHub Actions** | Scheduled data jobs, report generation |

## Examples

See the [examples](crates/tsumugi/examples/) directory:

### Basic
- [simple_workflow.rs](crates/tsumugi/examples/simple_workflow.rs) - Single-step workflow
- [order_workflow.rs](crates/tsumugi/examples/order_workflow.rs) - Multi-step with branching
- [user_scoring_workflow.rs](crates/tsumugi/examples/user_scoring_workflow.rs) - Data processing

### Real-World Patterns
- [etl_api_to_csv.rs](crates/tsumugi/examples/etl_api_to_csv.rs) - REST API to CSV (GitHub Actions friendly)
- [health_check_monitor.rs](crates/tsumugi/examples/health_check_monitor.rs) - Service health monitoring with retries
- [file_processing_pipeline.rs](crates/tsumugi/examples/file_processing_pipeline.rs) - Batch log file aggregation
- [data_validation_pipeline.rs](crates/tsumugi/examples/data_validation_pipeline.rs) - Multi-stage data validation
- [notification_dispatch.rs](crates/tsumugi/examples/notification_dispatch.rs) - Multi-channel notifications

Run examples:

```bash
# Basic
cargo run -p tsumugi --example simple_workflow

# Real-world patterns
cargo run -p tsumugi --example etl_api_to_csv
cargo run -p tsumugi --example health_check_monitor
cargo run -p tsumugi --example data_validation_pipeline
```

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
