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

use std::time::Duration;
use tsumugi::prelude::*;

/// The state shared by all steps of the monitor. Each field is filled in by
/// one step and required by the next.
#[derive(Default)]
struct MonitorState {
    /// Set by the load step.
    configs: Option<Vec<ServiceConfig>>,
    /// Set by the check step, moved into the report by the aggregate step.
    results: Option<Vec<ServiceHealth>>,
    /// Set by the aggregate step.
    report: Option<HealthReport>,
}

// Health check result for a single service
struct ServiceHealth {
    name: String,
    // Not read by this demo, which only prints the name.
    #[allow(dead_code)]
    url: String,
    status: HealthStatus,
    response_time_ms: u64,
    message: String,
}

#[derive(Debug, PartialEq)]
enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

// Aggregated health report
struct HealthReport {
    timestamp: String,
    services: Vec<ServiceHealth>,
    overall_status: HealthStatus,
}

// Service configuration
struct ServiceConfig {
    name: String,
    url: String,
    // Only used by the commented-out production request below.
    #[allow(dead_code)]
    timeout_ms: u64,
}

// Step 1: Load service configurations
struct LoadConfigStep;

#[async_trait]
impl Step<MonitorState> for LoadConfigStep {
    async fn run(&self, state: &mut MonitorState) -> StepResult {
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
        state.configs = Some(services);

        Ok(Next::step("check_services"))
    }
}

// Step 2: Check all services (with simulated retries)
struct CheckServicesStep;

#[async_trait]
impl Step<MonitorState> for CheckServicesStep {
    async fn run(&self, state: &mut MonitorState) -> StepResult {
        println!("Checking service health...");

        let configs = state
            .configs
            .as_ref()
            .ok_or("service configurations not loaded")?;

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
            let health = simulate_health_check(config);
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

        state.results = Some(results);

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
struct AggregateResultsStep;

#[async_trait]
impl Step<MonitorState> for AggregateResultsStep {
    async fn run(&self, state: &mut MonitorState) -> StepResult {
        println!("Aggregating health results...");

        let results = state.results.take().ok_or("health results missing")?;

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

        state.report = Some(report);

        Ok(Next::step("alert"))
    }
}

// Step 4: Send alerts if needed
struct AlertStep;

#[async_trait]
impl Step<MonitorState> for AlertStep {
    async fn run(&self, state: &mut MonitorState) -> StepResult {
        let report = state.report.as_ref().ok_or("health report missing")?;

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
struct ReportStep;

#[async_trait]
impl Step<MonitorState> for ReportStep {
    async fn run(&self, state: &mut MonitorState) -> StepResult {
        let report = state.report.as_ref().ok_or("health report missing")?;

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

    let workflow = WorkflowBuilder::<MonitorState>::new()
        .add_step("load_config", LoadConfigStep)
        .then(["check_services"])
        .add_step("check_services", CheckServicesStep)
        .then(["aggregate"])
        .add_step("aggregate", AggregateResultsStep)
        .then(["alert"])
        .add_step("alert", AlertStep)
        .then(["report"])
        .add_step("report", ReportStep)
        .terminal()
        .build()?;

    let mut state = MonitorState::default();

    println!("=== Health Check Monitor ===\n");

    match workflow.run(&mut state).await {
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
