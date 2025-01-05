# Tsumugi (紡)

[![Crates.io](https://img.shields.io/crates/v/tsumugi.svg)](https://crates.io/crates/tsumugi)
[![Documentation](https://docs.rs/tsumugi/badge.svg)](https://docs.rs/tsumugi)
[![License](https://img.shields.io/crates/l/tsumugi.svg)](LICENSE)

A lightweight and simple workflow engine for Rust, designed for building type-safe and maintainable workflows. The name "Tsumugi" (紡) means "to spin" or "to weave" in Japanese, representing how this engine elegantly weaves together different tasks into a robust workflow.

## Features

- 🪶 **Lightweight**: Minimal dependencies, focused on core workflow functionality
- 🎯 **Type-safe**: Generic type system for workflow data
- ⚡ **Async First**: Built with async-trait for asynchronous workflows
- 🛠️ **Configurable**: Per-step timeout settings (default: 30s)
- 📊 **Context Aware**: Type-safe context for data sharing between steps
- 🔄 **Error Handling**: Built-in timeout handling and error hooks
- 🔍 **Tracing**: Basic execution logging and step tracking

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
tsumugi = "0.1"
async-trait = "0.1"
```

Create a simple workflow:

```rust
use tsumugi::prelude::*;
use async_trait::async_trait;

// Define your step
define_step!(DataLoadStep);

#[async_trait]
impl Step<String> for DataLoadStep {
    async fn execute(&self, ctx: &mut Context<String>) -> Result<Option<String>, WorkflowError> {
        println!("Loading data...");
        ctx.insert("data", "Hello, World!".to_string());
        Ok(None)
    }
}

// Create and run workflow
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workflow = Workflow::builder()
        .add::<DataLoadStep>()
        .start_with_type::<DataLoadStep>()
        .build()?;

    let mut ctx = Context::new();
    workflow.execute(&mut ctx).await?;
    Ok(())
}
```

## Core Concepts

### Steps

Steps are the building blocks of workflows. Each step:
- Implements the `Step` trait with generic type parameter
- Can access and modify the workflow context
- Can specify the next step to execute via return value
- Has configurable timeout settings (default: 30s)
- Supports `on_success` and `on_failure` hooks

### Context

The `Context` type provides:
- Type-safe data storage with generic type parameter
- Key-value based data sharing between steps
- Basic execution tracking (elapsed time)
- Metadata storage for additional information

### Error Handling

Built-in error handling with:
- Step-specific errors (StepError, Timeout, StepNotFound, Configuration)
- Basic timeout handling with configurable duration (default: 30s)
- Error hooks with `on_success` and `on_failure` callbacks
- Error propagation with detailed error types

## Examples

Check out the [examples](examples/) directory for more complex use cases:

- [Simple Workflow](examples/simple_workflow.rs) - Basic workflow demonstration
- [Order Processing](examples/order_workflow.rs) - Complex business workflow
- [User Scoring](examples/user_scoring_workflow.rs) - Data processing pipeline
# tsumugi
