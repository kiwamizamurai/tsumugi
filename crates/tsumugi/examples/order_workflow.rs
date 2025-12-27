//! Multi-step order processing workflow with branching.
//!
//! Demonstrates:
//! - Heterogeneous context storage (different types without wrapper enum)
//! - Conditional branching between steps
//! - Complex data structures

#![allow(dead_code)]

use async_trait::async_trait;
use std::collections::HashMap;
use tsumugi::prelude::*;

// Data structures - stored directly in Context without wrapper enum
#[derive(Debug, Clone)]
struct Order {
    id: String,
    customer_id: String,
    items: Vec<OrderItem>,
    total_amount: f64,
    payment_method: PaymentMethod,
    shipping_address: Address,
}

#[derive(Debug, Clone)]
struct OrderItem {
    product_id: String,
    quantity: u32,
    price: f64,
}

#[derive(Debug, Clone)]
struct Address {
    street: String,
    city: String,
    country: String,
    postal_code: String,
}

#[derive(Debug, Clone)]
enum PaymentMethod {
    CreditCard {
        card_number: String,
        expiry: String,
    },
    BankTransfer {
        account_number: String,
        bank_code: String,
    },
}

#[derive(Debug, Clone)]
struct PaymentStatus {
    transaction_id: String,
    status: String,
}

#[derive(Debug, Clone)]
struct ShippingInfo {
    tracking_number: String,
    estimated_delivery: String,
}

// Step 1: Order Validation
#[derive(Debug)]
struct OrderValidationStep;

#[async_trait]
impl Step for OrderValidationStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Validating order...");

        let order = ctx
            .get::<Order>("order")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Order data not found".to_string(),
            })?;

        if order.items.is_empty() {
            return Err(WorkflowError::StepError {
                step_name: self.name(),
                details: "Order must contain at least one item".to_string(),
            });
        }

        if order.total_amount <= 0.0 {
            return Err(WorkflowError::StepError {
                step_name: self.name(),
                details: "Invalid order amount".to_string(),
            });
        }

        Ok(StepOutput::next("inventory_check"))
    }

    fn name(&self) -> StepName {
        StepName::new("OrderValidation")
    }
}

// Step 2: Inventory Check
#[derive(Debug)]
struct InventoryCheckStep;

#[async_trait]
impl Step for InventoryCheckStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Checking inventory...");

        let order = ctx
            .get::<Order>("order")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Order data not found".to_string(),
            })?;

        let inventory = ctx
            .get::<HashMap<String, u32>>("inventory")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Inventory data not found".to_string(),
            })?;

        for item in &order.items {
            let available =
                inventory
                    .get(&item.product_id)
                    .ok_or_else(|| WorkflowError::StepError {
                        step_name: self.name(),
                        details: format!("Product not found: {}", item.product_id),
                    })?;

            if available < &item.quantity {
                return Ok(StepOutput::next("pending_notification"));
            }
        }

        Ok(StepOutput::next("payment_processing"))
    }

    fn name(&self) -> StepName {
        StepName::new("InventoryCheck")
    }
}

// Step 3: Payment Processing
#[derive(Debug)]
struct PaymentProcessingStep;

#[async_trait]
impl Step for PaymentProcessingStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Processing payment...");

        let order = ctx
            .get::<Order>("order")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Order data not found".to_string(),
            })?;

        let (payment_status, next_step) = match &order.payment_method {
            PaymentMethod::CreditCard { .. } => (
                PaymentStatus {
                    transaction_id: "CC-TRANS-123".to_string(),
                    status: "SUCCESS".to_string(),
                },
                "shipping_arrangement",
            ),
            PaymentMethod::BankTransfer { .. } => (
                PaymentStatus {
                    transaction_id: "BT-TRANS-456".to_string(),
                    status: "PENDING".to_string(),
                },
                "pending_notification",
            ),
        };

        ctx.insert("payment_status", payment_status);
        Ok(StepOutput::next(next_step))
    }

    fn name(&self) -> StepName {
        StepName::new("PaymentProcessing")
    }
}

// Step 4: Shipping Arrangement
#[derive(Debug)]
struct ShippingArrangementStep;

#[async_trait]
impl Step for ShippingArrangementStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Arranging shipping...");

        let order = ctx
            .get::<Order>("order")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Order data not found".to_string(),
            })?;

        let shipping_info = ShippingInfo {
            tracking_number: format!("TRACK-{}", order.id),
            estimated_delivery: "2024-02-20".to_string(),
        };

        ctx.insert("shipping_info", shipping_info);
        Ok(StepOutput::next("success_notification"))
    }

    fn name(&self) -> StepName {
        StepName::new("ShippingArrangement")
    }
}

// Step 5: Success Notification
#[derive(Debug)]
struct SuccessNotificationStep;

#[async_trait]
impl Step for SuccessNotificationStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Sending success notification...");

        let order = ctx
            .get::<Order>("order")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Order data not found".to_string(),
            })?;

        let shipping_info =
            ctx.get::<ShippingInfo>("shipping_info")
                .ok_or_else(|| WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Shipping info not found".to_string(),
                })?;

        println!(
            "Order successful! Order ID: {}, Tracking: {}, ETA: {}",
            order.id, shipping_info.tracking_number, shipping_info.estimated_delivery
        );

        Ok(StepOutput::done())
    }

    fn name(&self) -> StepName {
        StepName::new("SuccessNotification")
    }
}

// Step 6: Pending Notification
#[derive(Debug)]
struct PendingNotificationStep;

#[async_trait]
impl Step for PendingNotificationStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Sending pending notification...");

        let order = ctx
            .get::<Order>("order")
            .ok_or_else(|| WorkflowError::StepError {
                step_name: self.name(),
                details: "Order data not found".to_string(),
            })?;

        println!(
            "Payment pending for Order ID: {}. Please complete the transfer.",
            order.id
        );

        Ok(StepOutput::done())
    }

    fn name(&self) -> StepName {
        StepName::new("PendingNotification")
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Initialize inventory - stored directly as HashMap
    let inventory: HashMap<String, u32> = HashMap::from([
        ("PROD-001".to_string(), 10),
        ("PROD-002".to_string(), 5),
        ("PROD-003".to_string(), 15),
    ]);

    // Create order - stored directly as Order
    let order = Order {
        id: "ORD-123".to_string(),
        customer_id: "CUST-456".to_string(),
        items: vec![
            OrderItem {
                product_id: "PROD-001".to_string(),
                quantity: 2,
                price: 29.99,
            },
            OrderItem {
                product_id: "PROD-002".to_string(),
                quantity: 1,
                price: 49.99,
            },
        ],
        total_amount: 109.97,
        payment_method: PaymentMethod::CreditCard {
            card_number: "4111-1111-1111-1111".to_string(),
            expiry: "12/25".to_string(),
        },
        shipping_address: Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            country: "USA".to_string(),
            postal_code: "12345".to_string(),
        },
    };

    let workflow = Workflow::builder()
        .add_step("order_validation", OrderValidationStep)
        .add_step("inventory_check", InventoryCheckStep)
        .add_step("payment_processing", PaymentProcessingStep)
        .add_step("shipping_arrangement", ShippingArrangementStep)
        .add_step("success_notification", SuccessNotificationStep)
        .add_step("pending_notification", PendingNotificationStep)
        .start_with("order_validation")
        .build()?;

    let mut ctx = Context::new();
    // Store different types directly - no wrapper enum needed!
    ctx.insert("order", order);
    ctx.insert("inventory", inventory);

    match workflow.execute(&mut ctx).await {
        Ok(()) => println!("\nWorkflow completed successfully"),
        Err(errors) => {
            for error in errors {
                eprintln!("Workflow failed: {:?}", error);
            }
        }
    }

    Ok(())
}
