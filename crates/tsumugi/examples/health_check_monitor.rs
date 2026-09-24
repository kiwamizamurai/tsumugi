//! Health Check Monitoring Workflow.
//!
//! This example demonstrates:
//! 1. Checking multiple service endpoints
//! 2. Aggregating health status
//! 3. Alerting on failures with retry logic
//!
//! Use cases:
//! - Uptime monitoring embedded in microservices
//! - Pre-deployment health verification in CI/CD
//! - Scheduled health reports via cron

#![allow(dead_code)]

use std::time::Duration;
use tsumugi::prelude::*;

// Health check result for a single service
#[derive(Debug, Clone)]
struct ServiceHealth {
    name: String,
    url: String,
    status: HealthStatus,
    response_time_ms: u64,
    message: String,
}

#[derive(Debug, Clone, PartialEq)]
enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

// Aggregated health report
#[derive(Debug, Clone)]
struct HealthReport {
    timestamp: String,
    services: Vec<ServiceHealth>,
    overall_status: HealthStatus,
}

// Service configuration
#[derive(Debug, Clone)]
struct ServiceConfig {
    name: String,
    url: String,
    timeout_ms: u64,
}

// Step 1: Load service configurations
#[derive(Debug)]
struct LoadConfigStep;

#[async_trait]
impl Step for LoadConfigStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Loading service configurations...");

        // In production, load from config file or environment
        let services = vec![
            ServiceConfig {
                name: "API Gateway".to_string(),
                url: "https://api.example.com/health".to_string(),
                timeout_ms: 5000,
            },
            ServiceConfig {
                name: "Database".to_string(),
                url: "https://db.example.com/health".to_string(),
                timeout_ms: 3000,
            },
            ServiceConfig {
                name: "Cache".to_string(),
                url: "https://cache.example.com/health".to_string(),
                timeout_ms: 2000,
            },
            ServiceConfig {
                name: "Message Queue".to_string(),
                url: "https://mq.example.com/health".to_string(),
                timeout_ms: 3000,
            },
        ];

        println!("  Loaded {} service configurations", services.len());
        ctx.insert("service_configs", services);

        Ok(Next::step("check_services"))
    }
}

// Step 2: Check all services (with simulated retries)
#[derive(Debug)]
struct CheckServicesStep;

#[async_trait]
impl Step for CheckServicesStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Checking service health...");

        let configs = ctx
            .require::<Vec<ServiceConfig>>("service_configs")?
            .clone();

        let mut results: Vec<ServiceHealth> = Vec::new();

        for config in configs {
            // In production, use reqwest with timeout:
            // let client = reqwest::Client::new();
            // let start = Instant::now();
            // let response = client
            //     .get(&config.url)
            //     .timeout(Duration::from_millis(config.timeout_ms))
            //     .send()
            //     .await;

            // Simulated health check results
            let health = simulate_health_check(&config);
            println!(
                "  {} [{}]: {:?} ({}ms)",
                health.name,
                if health.status == HealthStatus::Healthy {
                    "OK"
                } else {
                    "!!"
                },
                health.status,
                health.response_time_ms
            );
            results.push(health);
        }

        ctx.insert("health_results", results);

        Ok(Next::step("aggregate"))
    }

    fn retry_policy(&self) -> RetryPolicy {
        // 3 retries, starting at 500ms, max 5s, multiplier 2
        RetryPolicy::exponential(3, Duration::from_millis(500)).max_delay(Duration::from_secs(5))
    }

    fn timeout(&self) -> Option<Duration> {
        Some(Duration::from_secs(30))
    }
}

// Simulate health check (in production, make actual HTTP requests)
fn simulate_health_check(config: &ServiceConfig) -> ServiceHealth {
    // Simulate varying health statuses
    let (status, response_time, message) = match config.name.as_str() {
        "API Gateway" => (HealthStatus::Healthy, 45, "All endpoints responding"),
        "Database" => (HealthStatus::Healthy, 12, "Primary node active"),
        "Cache" => (HealthStatus::Degraded, 250, "High latency detected"),
        "Message Queue" => (HealthStatus::Healthy, 8, "Queue depth normal"),
        _ => (HealthStatus::Unhealthy, 0, "Service unreachable"),
    };

    ServiceHealth {
        name: config.name.clone(),
        url: config.url.clone(),
        status,
        response_time_ms: response_time,
        message: message.to_string(),
    }
}

// Step 3: Aggregate results into report
#[derive(Debug)]
struct AggregateResultsStep;

#[async_trait]
impl Step for AggregateResultsStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Aggregating health results...");

        let results = ctx.require::<Vec<ServiceHealth>>("health_results")?.clone();

        // Determine overall status
        let overall_status = if results.iter().any(|h| h.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else if results.iter().any(|h| h.status == HealthStatus::Degraded) {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let report = HealthReport {
            timestamp: "2024-01-15T12:00:00Z".to_string(),
            services: results,
            overall_status,
        };

        ctx.insert("health_report", report);

        Ok(Next::step("alert"))
    }
}

// Step 4: Send alerts if needed
#[derive(Debug)]
struct AlertStep;

#[async_trait]
impl Step for AlertStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        let report = ctx.require::<HealthReport>("health_report")?;

        match &report.overall_status {
            HealthStatus::Unhealthy => {
                println!("\nALERT: System is UNHEALTHY!");
                // In production: send to PagerDuty, Slack, etc.
                // slack::send_alert("System unhealthy", &report).await?;
            }
            HealthStatus::Degraded => {
                println!("\nWARNING: System is DEGRADED");
                // In production: send warning notification
            }
            HealthStatus::Healthy => {
                println!("\nSystem is healthy - no alerts needed");
            }
        }

        Ok(Next::step("report"))
    }
}

// Step 5: Generate report
#[derive(Debug)]
struct ReportStep;

#[async_trait]
impl Step for ReportStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        let report = ctx.require::<HealthReport>("health_report")?;

        println!("\n╔══════════════════════════════════════════╗");
        println!("║         HEALTH CHECK REPORT              ║");
        println!("╠══════════════════════════════════════════╣");
        println!("║ Timestamp: {}        ║", report.timestamp);
        println!(
            "║ Overall Status: {:?}                  ║",
            report.overall_status
        );
        println!("╠══════════════════════════════════════════╣");

        for service in &report.services {
            let status_icon = match service.status {
                HealthStatus::Healthy => "✓",
                HealthStatus::Degraded => "!",
                HealthStatus::Unhealthy => "✗",
            };
            println!(
                "║ [{}] {:<15} {:>4}ms              ║",
                status_icon, service.name, service.response_time_ms
            );
            println!("║     {}  ║", service.message);
        }

        println!("╚══════════════════════════════════════════╝");

        Ok(Next::Done)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = Workflow::builder()
        .add_step("load_config", LoadConfigStep)
        .add_step("check_services", CheckServicesStep)
        .add_step("aggregate", AggregateResultsStep)
        .add_step("alert", AlertStep)
        .add_step("report", ReportStep)
        .start_with("load_config")
        .build()?;

    let mut ctx = Context::new();

    println!("=== Health Check Monitor ===\n");

    match workflow.run(&mut ctx).await {
        Ok(_) => {
            println!("\nHealth check completed successfully!");
        }
        Err(err) => {
            eprintln!("Health check failed: {}", err);
            eprintln!("{}", err.report());
            std::process::exit(1);
        }
    }

    Ok(())
}
