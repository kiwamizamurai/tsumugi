use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tsumugi::prelude::*;

// Data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Order {
    id: String,
    customer_id: String,
    items: Vec<OrderItem>,
    total_amount: f64,
    payment_method: PaymentMethod,
    shipping_address: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderItem {
    product_id: String,
    quantity: u32,
    price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Address {
    street: String,
    city: String,
    country: String,
    postal_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum PaymentMethod {
    CreditCard(CreditCardInfo),
    BankTransfer(BankInfo),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreditCardInfo {
    card_number: String,
    expiry: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BankInfo {
    account_number: String,
    bank_code: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum WorkflowData {
    Order(Order),
    InventoryStatus(HashMap<String, u32>),
    PaymentStatus(PaymentStatus),
    ShippingInfo(ShippingInfo),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
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
define_step!(OrderValidationStep);

#[async_trait]
impl Step<WorkflowData> for OrderValidationStep {
    async fn execute(
        &self,
        ctx: &mut Context<WorkflowData>,
    ) -> Result<Option<StepName>, WorkflowError> {
        println!("Validating order...");

        let order = match ctx.get("order") {
            Some(WorkflowData::Order(order)) => order,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Order data not found".to_string(),
                })
            }
        };

        // Validate order
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

        Ok(Some(StepName::new("InventoryCheckStep")))
    }
}

// Step 2: Inventory Check
define_step!(InventoryCheckStep);

#[async_trait]
impl Step<WorkflowData> for InventoryCheckStep {
    async fn execute(
        &self,
        ctx: &mut Context<WorkflowData>,
    ) -> Result<Option<StepName>, WorkflowError> {
        let order = match ctx.get("order") {
            Some(WorkflowData::Order(order)) => order,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Order data not found".to_string(),
                })
            }
        };

        let inventory = match ctx.get("inventory") {
            Some(WorkflowData::InventoryStatus(inventory)) => inventory,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Inventory data not found".to_string(),
                })
            }
        };

        // 在庫チェックロジック
        for item in &order.items {
            let available = inventory
                .get(&item.product_id)
                .ok_or(WorkflowError::StepError {
                    step_name: self.name(),
                    details: format!("Product not found: {}", item.product_id),
                })?;

            if available < &item.quantity {
                return Ok(Some(StepName::new("PendingNotificationStep")));
            }
        }

        Ok(Some(StepName::new("PaymentProcessingStep")))
    }
}

// Step 3: Payment Processing
define_step!(PaymentProcessingStep);

#[async_trait]
impl Step<WorkflowData> for PaymentProcessingStep {
    async fn execute(
        &self,
        ctx: &mut Context<WorkflowData>,
    ) -> Result<Option<StepName>, WorkflowError> {
        println!("Processing payment...");

        let order = match ctx.get("order") {
            Some(WorkflowData::Order(order)) => order.clone(),
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Order data not found".to_string(),
                })
            }
        };

        // Simulate payment processing
        let payment_status = match &order.payment_method {
            PaymentMethod::CreditCard(_) => PaymentStatus {
                transaction_id: "CC-TRANS-123".to_string(),
                status: "SUCCESS".to_string(),
            },
            PaymentMethod::BankTransfer(_) => PaymentStatus {
                transaction_id: "BT-TRANS-456".to_string(),
                status: "PENDING".to_string(),
            },
        };

        ctx.insert(
            "payment_status",
            WorkflowData::PaymentStatus(payment_status),
        );

        match &order.payment_method {
            PaymentMethod::CreditCard(_) => Ok(Some(StepName::new("ShippingArrangementStep"))),
            PaymentMethod::BankTransfer(_) => Ok(Some(StepName::new("PendingNotificationStep"))),
        }
    }
}

// Step 4: Shipping Arrangement
define_step!(ShippingArrangementStep);

#[async_trait]
impl Step<WorkflowData> for ShippingArrangementStep {
    async fn execute(
        &self,
        ctx: &mut Context<WorkflowData>,
    ) -> Result<Option<StepName>, WorkflowError> {
        println!("Arranging shipping...");

        let order = match ctx.get("order") {
            Some(WorkflowData::Order(order)) => order,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Order data not found".to_string(),
                })
            }
        };

        // Simulate shipping arrangement
        let shipping_info = ShippingInfo {
            tracking_number: format!("TRACK-{}", order.id),
            estimated_delivery: "2024-02-20".to_string(),
        };

        ctx.insert("shipping_info", WorkflowData::ShippingInfo(shipping_info));
        Ok(Some(StepName::new("SuccessNotificationStep")))
    }
}

// Step 5: Success Notification
define_step!(SuccessNotificationStep);

#[async_trait]
impl Step<WorkflowData> for SuccessNotificationStep {
    async fn execute(
        &self,
        ctx: &mut Context<WorkflowData>,
    ) -> Result<Option<StepName>, WorkflowError> {
        println!("Sending success notification...");

        let order = match ctx.get("order") {
            Some(WorkflowData::Order(order)) => order,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Order data not found".to_string(),
                })
            }
        };

        let shipping_info = match ctx.get("shipping_info") {
            Some(WorkflowData::ShippingInfo(info)) => info,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Shipping info not found".to_string(),
                })
            }
        };

        println!(
            "Order successful! Order ID: {}, Tracking Number: {}, Estimated Delivery: {}",
            order.id, shipping_info.tracking_number, shipping_info.estimated_delivery
        );

        Ok(None)
    }
}

// Step 6: Pending Payment Notification
define_step!(PendingNotificationStep);

#[async_trait]
impl Step<WorkflowData> for PendingNotificationStep {
    async fn execute(
        &self,
        ctx: &mut Context<WorkflowData>,
    ) -> Result<Option<StepName>, WorkflowError> {
        println!("Sending pending payment notification...");

        let order = match ctx.get("order") {
            Some(WorkflowData::Order(order)) => order,
            _ => {
                return Err(WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Order data not found".to_string(),
                })
            }
        };

        println!(
            "Payment pending for Order ID: {}. Please complete the bank transfer.",
            order.id
        );

        Ok(None)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Initialize test inventory
    let initial_inventory = HashMap::from([
        ("PROD-001".to_string(), 10),
        ("PROD-002".to_string(), 5),
        ("PROD-003".to_string(), 15),
    ]);

    // Create test order
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
        payment_method: PaymentMethod::CreditCard(CreditCardInfo {
            card_number: "4111-1111-1111-1111".to_string(),
            expiry: "12/25".to_string(),
        }),
        shipping_address: Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            country: "USA".to_string(),
            postal_code: "12345".to_string(),
        },
    };

    let workflow = Workflow::builder()
        .add::<OrderValidationStep>()
        .add::<InventoryCheckStep>()
        .add::<PaymentProcessingStep>()
        .add::<ShippingArrangementStep>()
        .add::<SuccessNotificationStep>()
        .add::<PendingNotificationStep>()
        .start_with_type::<OrderValidationStep>()
        .build()?;

    let mut ctx = Context::new();
    ctx.insert("order", WorkflowData::Order(order));
    ctx.insert(
        "inventory",
        WorkflowData::InventoryStatus(initial_inventory),
    );

    match workflow.execute(&mut ctx).await {
        Ok(()) => println!("Workflow completed successfully"),
        Err(errors) => {
            for error in errors {
                println!("Workflow failed: {:?}", error);
            }
        }
    }

    Ok(())
}
