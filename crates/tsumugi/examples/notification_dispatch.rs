//! Multi-Channel Notification Dispatch Workflow.
//!
//! This example demonstrates notification delivery:
//! 1. Template rendering
//! 2. Multi-channel dispatch (Email, Slack, Webhook)
//! 3. Delivery tracking
//! 4. Failure handling with retries
//!
//! Use cases:
//! - Alert notifications
//! - User onboarding emails
//! - System status updates
//! - Scheduled report delivery

// Some fields and variants only illustrate a realistic data model.
#![allow(dead_code)]

use std::time::Duration;
use tsumugi::prelude::*;

// Notification request
#[derive(Debug, Clone)]
struct NotificationRequest {
    id: String,
    template: String,
    recipient: Recipient,
    channels: Vec<Channel>,
    priority: Priority,
    context: NotificationContext,
}

#[derive(Debug, Clone)]
struct Recipient {
    name: String,
    email: Option<String>,
    slack_id: Option<String>,
    webhook_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Channel {
    Email,
    Slack,
    Webhook,
}

#[derive(Debug, Clone, PartialEq)]
enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone)]
struct NotificationContext {
    subject: String,
    body: String,
    metadata: std::collections::HashMap<String, String>,
}

// Rendered notification
#[derive(Debug, Clone)]
struct RenderedNotification {
    subject: String,
    body: String,
    html_body: Option<String>,
}

// Delivery result
#[derive(Debug, Clone)]
struct DeliveryResult {
    channel: Channel,
    success: bool,
    message_id: Option<String>,
    error: Option<String>,
}

#[derive(Debug)]
struct DeliveryReport {
    notification_id: String,
    results: Vec<DeliveryResult>,
    all_succeeded: bool,
}

/// The state shared by all steps of the workflow.
#[derive(Default)]
struct Dispatch {
    request: Option<NotificationRequest>,
    rendered: Option<RenderedNotification>,
    deliveries: Vec<DeliveryResult>,
    report: Option<DeliveryReport>,
}

// Step 1: Load notification request
struct LoadRequestStep;

#[async_trait]
impl Step<Dispatch> for LoadRequestStep {
    async fn run(&self, state: &mut Dispatch) -> StepResult {
        println!("Loading notification request...");

        // In production, receive from queue or API
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("deployment_id".to_string(), "deploy-123".to_string());
        metadata.insert("environment".to_string(), "production".to_string());

        let request = NotificationRequest {
            id: "notif-001".to_string(),
            template: "deployment_complete".to_string(),
            recipient: Recipient {
                name: "DevOps Team".to_string(),
                email: Some("devops@example.com".to_string()),
                slack_id: Some("#deployments".to_string()),
                webhook_url: Some("https://hooks.example.com/notify".to_string()),
            },
            channels: vec![Channel::Email, Channel::Slack, Channel::Webhook],
            priority: Priority::High,
            context: NotificationContext {
                subject: "Deployment Complete".to_string(),
                body: "Deployment {deployment_id} to {environment} completed successfully."
                    .to_string(),
                metadata,
            },
        };

        println!("  Notification ID: {}", request.id);
        println!("  Channels: {:?}", request.channels);
        println!("  Priority: {:?}", request.priority);

        state.request = Some(request);

        Ok(Next::step("render"))
    }
}

// Step 2: Render template
struct RenderTemplateStep;

#[async_trait]
impl Step<Dispatch> for RenderTemplateStep {
    async fn run(&self, state: &mut Dispatch) -> StepResult {
        println!("Rendering notification template...");

        let request = state
            .request
            .as_ref()
            .ok_or("notification request not loaded")?;

        // Simple template rendering (in production, use handlebars or tera)
        let mut body = request.context.body.clone();
        for (key, value) in &request.context.metadata {
            body = body.replace(&format!("{{{}}}", key), value);
        }

        let rendered = RenderedNotification {
            subject: request.context.subject.clone(),
            html_body: Some(format!(
                "<html><body><h1>{}</h1><p>{}</p></body></html>",
                request.context.subject, body
            )),
            body,
        };

        println!("  Subject: {}", rendered.subject);
        println!("  Body: {}", rendered.body);

        state.rendered = Some(rendered);

        Ok(Next::step("dispatch"))
    }
}

// Step 3: Dispatch to all channels
struct DispatchStep;

#[async_trait]
impl Step<Dispatch> for DispatchStep {
    async fn run(&self, state: &mut Dispatch) -> StepResult {
        println!("Dispatching notifications...");

        let request = state
            .request
            .as_ref()
            .ok_or("notification request not loaded")?;
        let rendered = state.rendered.as_ref().ok_or("notification not rendered")?;

        let mut results = Vec::new();

        for channel in &request.channels {
            let result = dispatch_to_channel(channel, &request.recipient, rendered);
            println!(
                "  {} {:?} -> {}",
                if result.success { "[OK]" } else { "[FAIL]" },
                channel,
                result.message_id.as_deref().unwrap_or("N/A")
            );
            results.push(result);
        }

        state.deliveries = results;

        Ok(Next::step("report"))
    }

    fn retry_policy(&self) -> RetryPolicy {
        // 3 retries, starting at 1s, max 10s, multiplier 2
        RetryPolicy::exponential(3, Duration::from_secs(1)).max_delay(Duration::from_secs(10))
    }

    fn timeout(&self) -> Option<Duration> {
        Some(Duration::from_secs(30))
    }
}

// Simulate channel dispatch
fn dispatch_to_channel(
    channel: &Channel,
    recipient: &Recipient,
    _notification: &RenderedNotification,
) -> DeliveryResult {
    // In production, use actual delivery services:
    // - Email: SMTP, SendGrid, AWS SES
    // - Slack: Slack API
    // - Webhook: HTTP POST

    match channel {
        Channel::Email => {
            if recipient.email.is_some() {
                DeliveryResult {
                    channel: Channel::Email,
                    success: true,
                    message_id: Some("email-msg-12345".to_string()),
                    error: None,
                }
            } else {
                DeliveryResult {
                    channel: Channel::Email,
                    success: false,
                    message_id: None,
                    error: Some("No email address configured".to_string()),
                }
            }
        }
        Channel::Slack => {
            if recipient.slack_id.is_some() {
                DeliveryResult {
                    channel: Channel::Slack,
                    success: true,
                    message_id: Some("slack-ts-1705312800.123456".to_string()),
                    error: None,
                }
            } else {
                DeliveryResult {
                    channel: Channel::Slack,
                    success: false,
                    message_id: None,
                    error: Some("No Slack channel configured".to_string()),
                }
            }
        }
        Channel::Webhook => {
            if recipient.webhook_url.is_some() {
                DeliveryResult {
                    channel: Channel::Webhook,
                    success: true,
                    message_id: Some("webhook-req-abc123".to_string()),
                    error: None,
                }
            } else {
                DeliveryResult {
                    channel: Channel::Webhook,
                    success: false,
                    message_id: None,
                    error: Some("No webhook URL configured".to_string()),
                }
            }
        }
    }
}

// Step 4: Generate delivery report
struct ReportStep;

#[async_trait]
impl Step<Dispatch> for ReportStep {
    async fn run(&self, state: &mut Dispatch) -> StepResult {
        let request = state
            .request
            .as_ref()
            .ok_or("notification request not loaded")?;

        let results = std::mem::take(&mut state.deliveries);
        let all_succeeded = results.iter().all(|r| r.success);

        let report = DeliveryReport {
            notification_id: request.id.clone(),
            results,
            all_succeeded,
        };

        println!("\n┌─────────────────────────────────────────────────┐");
        println!("│        NOTIFICATION DELIVERY REPORT             │");
        println!("├─────────────────────────────────────────────────┤");
        println!(
            "│ Notification ID: {}                      │",
            report.notification_id
        );
        println!(
            "│ Status: {}                               │",
            if report.all_succeeded {
                "ALL DELIVERED"
            } else {
                "PARTIAL FAIL "
            }
        );
        println!("├─────────────────────────────────────────────────┤");

        for result in &report.results {
            let status = if result.success { "OK  " } else { "FAIL" };
            let channel = format!("{:?}", result.channel);
            println!(
                "│ [{}] {:<10}                              │",
                status, channel
            );
            if let Some(msg_id) = &result.message_id {
                println!("│       Message ID: {}          │", msg_id);
            }
            if let Some(error) = &result.error {
                println!("│       Error: {}              │", error);
            }
        }

        println!("└─────────────────────────────────────────────────┘");

        state.report = Some(report);

        Ok(Next::Done)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = WorkflowBuilder::<Dispatch>::new()
        .add_step("load", LoadRequestStep)
        .then(["render"])
        .add_step("render", RenderTemplateStep)
        .then(["dispatch"])
        .add_step("dispatch", DispatchStep)
        .then(["report"])
        .add_step("report", ReportStep)
        .terminal()
        .build()?;

    let mut state = Dispatch::default();

    println!("=== Notification Dispatch Workflow ===\n");

    if let Err(err) = workflow.run(&mut state).await {
        eprintln!("Notification workflow failed: {}", err);
        eprintln!("{}", err.report());
        std::process::exit(1);
    }

    let report = state.report.ok_or("workflow produced no delivery report")?;
    if report.all_succeeded {
        println!("\nAll notifications delivered successfully!");
    } else {
        println!("\nSome notifications failed to deliver.");
        std::process::exit(1);
    }

    Ok(())
}
