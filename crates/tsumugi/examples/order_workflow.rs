//! Multi-step order processing workflow with branching.
//!
//! Demonstrates:
//! - Heterogeneous context storage (different types without wrapper enum)
//! - Conditional branching between steps
//! - Complex data structures

#![allow(dead_code)]

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
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Validating order...");

        let order = ctx.require::<Order>("order")?;

        if order.items.is_empty() {
            return Err("Order must contain at least one item".into());
        }

        if order.total_amount <= 0.0 {
            return Err("Invalid order amount".into());
        }

        Ok(Next::step("inventory_check"))
    }
}

// Step 2: Inventory Check
#[derive(Debug)]
struct InventoryCheckStep;

#[async_trait]
impl Step for InventoryCheckStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Checking inventory...");

        let order = ctx.require::<Order>("order")?;

        let inventory = ctx.require::<HashMap<String, u32>>("inventory")?;

        for item in &order.items {
            let available = inventory
                .get(&item.product_id)
                .ok_or_else(|| format!("Product not found: {}", item.product_id))?;

            if available < &item.quantity {
                return Ok(Next::step("pending_notification"));
            }
        }

        Ok(Next::step("payment_processing"))
    }
}

// Step 3: Payment Processing
#[derive(Debug)]
struct PaymentProcessingStep;

#[async_trait]
impl Step for PaymentProcessingStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Processing payment...");

        let order = ctx.require::<Order>("order")?;

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
        Ok(Next::step(next_step))
    }
}

// Step 4: Shipping Arrangement
#[derive(Debug)]
struct ShippingArrangementStep;

#[async_trait]
impl Step for ShippingArrangementStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Arranging shipping...");

        let order = ctx.require::<Order>("order")?;

        let shipping_info = ShippingInfo {
            tracking_number: format!("TRACK-{}", order.id),
            estimated_delivery: "2024-02-20".to_string(),
        };

        ctx.insert("shipping_info", shipping_info);
        Ok(Next::step("success_notification"))
    }
}

// Step 5: Success Notification
#[derive(Debug)]
struct SuccessNotificationStep;

#[async_trait]
impl Step for SuccessNotificationStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Sending success notification...");

        let order = ctx.require::<Order>("order")?;

        let shipping_info = ctx.require::<ShippingInfo>("shipping_info")?;

        println!(
            "Order successful! Order ID: {}, Tracking: {}, ETA: {}",
            order.id, shipping_info.tracking_number, shipping_info.estimated_delivery
        );

        Ok(Next::Done)
    }
}

// Step 6: Pending Notification
#[derive(Debug)]
struct PendingNotificationStep;

#[async_trait]
impl Step for PendingNotificationStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Sending pending notification...");

        let order = ctx.require::<Order>("order")?;

        println!(
            "Payment pending for Order ID: {}. Please complete the transfer.",
            order.id
        );

        Ok(Next::Done)
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

    match workflow.run(&mut ctx).await {
        Ok(_) => println!("\nWorkflow completed successfully"),
        Err(err) => {
            eprintln!("Workflow failed: {}", err);
            eprintln!("{}", err.report());
        }
    }

    Ok(())
}
