//! Multi-step order processing workflow with branching.
//!
//! Demonstrates:
//! - A dedicated state struct holding the order, inventory and step outputs
//! - Conditional branching between steps, declared with `then`
//! - Complex data structures

// Several fields only exist to make the order data realistic and are never read.
#![allow(dead_code)]

use std::collections::HashMap;
use tsumugi::prelude::*;

// Data structures
#[derive(Debug)]
struct Order {
    id: String,
    customer_id: String,
    items: Vec<OrderItem>,
    total_amount: f64,
    payment_method: PaymentMethod,
    shipping_address: Address,
}

#[derive(Debug)]
struct OrderItem {
    product_id: String,
    quantity: u32,
    price: f64,
}

#[derive(Debug)]
struct Address {
    street: String,
    city: String,
    country: String,
    postal_code: String,
}

#[derive(Debug)]
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

#[derive(Debug)]
struct PaymentStatus {
    transaction_id: String,
    status: String,
}

#[derive(Debug)]
struct ShippingInfo {
    tracking_number: String,
    estimated_delivery: String,
}

/// The state shared by all steps. The order and inventory are provided up
/// front; the payment status and shipping info are produced by steps.
struct OrderState {
    order: Order,
    inventory: HashMap<String, u32>,
    payment_status: Option<PaymentStatus>,
    shipping_info: Option<ShippingInfo>,
}

// Step 1: Order Validation
struct OrderValidationStep;

#[async_trait]
impl Step<OrderState> for OrderValidationStep {
    async fn run(&self, state: &mut OrderState) -> StepResult {
        println!("Validating order...");

        if state.order.items.is_empty() {
            return Err("Order must contain at least one item".into());
        }

        if state.order.total_amount <= 0.0 {
            return Err("Invalid order amount".into());
        }

        Ok(Next::step("inventory_check"))
    }
}

// Step 2: Inventory Check
struct InventoryCheckStep;

#[async_trait]
impl Step<OrderState> for InventoryCheckStep {
    async fn run(&self, state: &mut OrderState) -> StepResult {
        println!("Checking inventory...");

        for item in &state.order.items {
            let available = state
                .inventory
                .get(&item.product_id)
                .ok_or_else(|| format!("Product not found: {}", item.product_id))?;

            if *available < item.quantity {
                return Ok(Next::step("pending_notification"));
            }
        }

        Ok(Next::step("payment_processing"))
    }
}

// Step 3: Payment Processing
struct PaymentProcessingStep;

#[async_trait]
impl Step<OrderState> for PaymentProcessingStep {
    async fn run(&self, state: &mut OrderState) -> StepResult {
        println!("Processing payment...");

        let (payment_status, next_step) = match &state.order.payment_method {
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

        state.payment_status = Some(payment_status);
        Ok(Next::step(next_step))
    }
}

// Step 4: Shipping Arrangement
struct ShippingArrangementStep;

#[async_trait]
impl Step<OrderState> for ShippingArrangementStep {
    async fn run(&self, state: &mut OrderState) -> StepResult {
        println!("Arranging shipping...");

        state.shipping_info = Some(ShippingInfo {
            tracking_number: format!("TRACK-{}", state.order.id),
            estimated_delivery: "2024-02-20".to_string(),
        });

        Ok(Next::step("success_notification"))
    }
}

// Step 5: Success Notification
struct SuccessNotificationStep;

#[async_trait]
impl Step<OrderState> for SuccessNotificationStep {
    async fn run(&self, state: &mut OrderState) -> StepResult {
        println!("Sending success notification...");

        let shipping_info = state
            .shipping_info
            .as_ref()
            .ok_or("Shipping has not been arranged")?;

        println!(
            "Order successful! Order ID: {}, Tracking: {}, ETA: {}",
            state.order.id, shipping_info.tracking_number, shipping_info.estimated_delivery
        );

        Ok(Next::Done)
    }
}

// Step 6: Pending Notification
struct PendingNotificationStep;

#[async_trait]
impl Step<OrderState> for PendingNotificationStep {
    async fn run(&self, state: &mut OrderState) -> StepResult {
        println!("Sending pending notification...");

        println!(
            "Payment pending for Order ID: {}. Please complete the transfer.",
            state.order.id
        );

        Ok(Next::Done)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let inventory = HashMap::from([
        ("PROD-001".to_string(), 10),
        ("PROD-002".to_string(), 5),
        ("PROD-003".to_string(), 15),
    ]);

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

    let workflow = WorkflowBuilder::<OrderState>::new()
        .add_step("order_validation", OrderValidationStep)
        .then(["inventory_check"])
        .add_step("inventory_check", InventoryCheckStep)
        .then(["payment_processing", "pending_notification"])
        .add_step("payment_processing", PaymentProcessingStep)
        .then(["shipping_arrangement", "pending_notification"])
        .add_step("shipping_arrangement", ShippingArrangementStep)
        .then(["success_notification"])
        .add_step("success_notification", SuccessNotificationStep)
        .terminal()
        .add_step("pending_notification", PendingNotificationStep)
        .terminal()
        .build()?;

    let mut state = OrderState {
        order,
        inventory,
        payment_status: None,
        shipping_info: None,
    };

    match workflow.run(&mut state).await {
        Ok(_) => println!("\nWorkflow completed successfully"),
        Err(err) => {
            eprintln!("Workflow failed: {}", err);
            eprintln!("{}", err.report());
        }
    }

    Ok(())
}
