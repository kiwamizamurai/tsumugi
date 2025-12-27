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

#![allow(dead_code)]

use async_trait::async_trait;
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

#[derive(Debug, Clone, Default)]
struct DeliveryReport {
    notification_id: String,
    results: Vec<DeliveryResult>,
    all_succeeded: bool,
}

// Step 1: Load notification request
#[derive(Debug)]
struct LoadRequestStep;

#[async_trait]
impl Step for LoadRequestStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
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

        ctx.insert("notification_request", request);

        Ok(StepOutput::next("render"))
    }

    fn name(&self) -> StepName {
        StepName::new("LoadRequest")
    }
}

// Step 2: Render template
#[derive(Debug)]
struct RenderTemplateStep;

#[async_trait]
impl Step for RenderTemplateStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Rendering notification template...");

        let request = ctx
            .get::<NotificationRequest>("notification_request")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Notification request not found".to_string(),
            })?;

        // Simple template rendering (in production, use handlebars or tera)
        let mut body = request.context.body.clone();
        for (key, value) in &request.context.metadata {
            body = body.replace(&format!("{{{}}}", key), value);
        }

        let rendered = RenderedNotification {
            subject: request.context.subject.clone(),
            body: body.clone(),
            html_body: Some(format!(
                "<html><body><h1>{}</h1><p>{}</p></body></html>",
                request.context.subject, body
            )),
        };

        println!("  Subject: {}", rendered.subject);
        println!("  Body: {}", rendered.body);

        ctx.insert("rendered_notification", rendered);

        Ok(StepOutput::next("dispatch"))
    }

    fn name(&self) -> StepName {
        StepName::new("RenderTemplate")
    }
}

// Step 3: Dispatch to all channels
#[derive(Debug)]
struct DispatchStep;

#[async_trait]
impl Step for DispatchStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Dispatching notifications...");

        let request = ctx
            .get::<NotificationRequest>("notification_request")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Request not found".to_string(),
            })?
            .clone();

        let rendered = ctx
            .get::<RenderedNotification>("rendered_notification")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Rendered notification not found".to_string(),
            })?
            .clone();

        let mut results: Vec<DeliveryResult> = Vec::new();

        for channel in &request.channels {
            let result = dispatch_to_channel(channel, &request.recipient, &rendered);
            println!(
                "  {} {} -> {}",
                if result.success { "[OK]" } else { "[FAIL]" },
                format!("{:?}", channel),
                result.message_id.as_deref().unwrap_or("N/A")
            );
            results.push(result);
        }

        ctx.insert("delivery_results", results);

        Ok(StepOutput::next("report"))
    }

    fn name(&self) -> StepName {
        StepName::new("Dispatch")
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
#[derive(Debug)]
struct ReportStep;

#[async_trait]
impl Step for ReportStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        let request = ctx
            .get::<NotificationRequest>("notification_request")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Request not found".to_string(),
            })?;

        let results = ctx
            .get::<Vec<DeliveryResult>>("delivery_results")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Delivery results not found".to_string(),
            })?
            .clone();

        let all_succeeded = results.iter().all(|r| r.success);

        let report = DeliveryReport {
            notification_id: request.id.clone(),
            results: results.clone(),
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
            if all_succeeded {
                "ALL DELIVERED"
            } else {
                "PARTIAL FAIL "
            }
        );
        println!("├─────────────────────────────────────────────────┤");

        for result in &results {
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

        ctx.insert("delivery_report", report);

        Ok(StepOutput::done())
    }

    fn name(&self) -> StepName {
        StepName::new("Report")
    }
}

// Implement retry for dispatch step
impl Retryable for DispatchStep {
    fn retry_policy(&self) -> RetryPolicy {
        // 3 retries, starting at 1s, max 10s, multiplier 2
        RetryPolicy::exponential_backoff(3, Duration::from_secs(1), Duration::from_secs(10), 2)
            .unwrap_or(RetryPolicy::None)
    }
}

// Implement timeout for dispatch step
impl WithTimeout for DispatchStep {
    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = Workflow::builder()
        .add_step("load", LoadRequestStep)
        .add_step("render", RenderTemplateStep)
        .add_retryable("dispatch", DispatchStep)
        .add_step("report", ReportStep)
        .start_with("load")
        .build()?;

    let mut ctx = Context::new();

    println!("=== Notification Dispatch Workflow ===\n");

    match workflow.execute(&mut ctx).await {
        Ok(()) => {
            let report = ctx.get::<DeliveryReport>("delivery_report");
            if let Some(r) = report {
                if r.all_succeeded {
                    println!("\nAll notifications delivered successfully!");
                } else {
                    println!("\nSome notifications failed to deliver.");
                    std::process::exit(1);
                }
            }
        }
        Err(errors) => {
            for error in errors {
                eprintln!("Notification workflow failed: {:?}", error);
            }
            std::process::exit(1);
        }
    }

    Ok(())
}
