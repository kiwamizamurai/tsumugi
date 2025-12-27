//! Data Validation Pipeline.
//!
//! This example demonstrates multi-stage data validation:
//! 1. Schema validation
//! 2. Business rule validation
//! 3. Cross-reference validation
//! 4. Report generation
//!
//! Use cases:
//! - CI/CD data quality gates
//! - Import data validation before database insert
//! - Configuration file validation
//! - API request validation pipeline

#![allow(dead_code)]

use async_trait::async_trait;
use tsumugi::prelude::*;

// Input data to validate
#[derive(Debug, Clone)]
struct ImportData {
    products: Vec<Product>,
    categories: Vec<Category>,
}

#[derive(Debug, Clone)]
struct Product {
    id: String,
    name: String,
    price: f64,
    category_id: String,
    stock: i32,
}

#[derive(Debug, Clone)]
struct Category {
    id: String,
    name: String,
    parent_id: Option<String>,
}

// Validation result
#[derive(Debug, Clone)]
struct ValidationError {
    field: String,
    message: String,
    severity: Severity,
}

#[derive(Debug, Clone, PartialEq)]
enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Default)]
struct ValidationResult {
    errors: Vec<ValidationError>,
    warnings: Vec<ValidationError>,
    passed: bool,
}

impl ValidationResult {
    fn add_error(&mut self, field: &str, message: &str) {
        self.errors.push(ValidationError {
            field: field.to_string(),
            message: message.to_string(),
            severity: Severity::Error,
        });
    }

    fn add_warning(&mut self, field: &str, message: &str) {
        self.warnings.push(ValidationError {
            field: field.to_string(),
            message: message.to_string(),
            severity: Severity::Warning,
        });
    }

    fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

// Step 1: Load data
#[derive(Debug)]
struct LoadDataStep;

#[async_trait]
impl Step for LoadDataStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Loading import data...");

        // In production, load from file or API
        let data = ImportData {
            products: vec![
                Product {
                    id: "P001".to_string(),
                    name: "Laptop".to_string(),
                    price: 999.99,
                    category_id: "CAT001".to_string(),
                    stock: 50,
                },
                Product {
                    id: "P002".to_string(),
                    name: "".to_string(), // Invalid: empty name
                    price: 49.99,
                    category_id: "CAT002".to_string(),
                    stock: 100,
                },
                Product {
                    id: "P003".to_string(),
                    name: "Headphones".to_string(),
                    price: -29.99, // Invalid: negative price
                    category_id: "CAT001".to_string(),
                    stock: 200,
                },
                Product {
                    id: "P004".to_string(),
                    name: "Keyboard".to_string(),
                    price: 79.99,
                    category_id: "CAT999".to_string(), // Invalid: non-existent category
                    stock: -5,                         // Invalid: negative stock
                },
                Product {
                    id: "P005".to_string(),
                    name: "Mouse".to_string(),
                    price: 29.99,
                    category_id: "CAT002".to_string(),
                    stock: 0, // Warning: zero stock
                },
            ],
            categories: vec![
                Category {
                    id: "CAT001".to_string(),
                    name: "Electronics".to_string(),
                    parent_id: None,
                },
                Category {
                    id: "CAT002".to_string(),
                    name: "Accessories".to_string(),
                    parent_id: Some("CAT001".to_string()),
                },
            ],
        };

        println!(
            "  Loaded {} products, {} categories",
            data.products.len(),
            data.categories.len()
        );

        ctx.insert("import_data", data);
        ctx.insert("validation_result", ValidationResult::default());

        Ok(StepOutput::next("schema_validation"))
    }

    fn name(&self) -> StepName {
        StepName::new("LoadData")
    }
}

// Step 2: Schema validation
#[derive(Debug)]
struct SchemaValidationStep;

#[async_trait]
impl Step for SchemaValidationStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Running schema validation...");

        let data =
            ctx.get::<ImportData>("import_data")
                .ok_or_else(|| WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Import data not found".to_string(),
                })?;

        let mut result = ValidationResult::default();

        // Validate products schema
        for product in &data.products {
            if product.id.is_empty() {
                result.add_error(&format!("product.{}.id", product.id), "ID cannot be empty");
            }
            if product.name.is_empty() {
                result.add_error(
                    &format!("product.{}.name", product.id),
                    "Name cannot be empty",
                );
            }
            if product.category_id.is_empty() {
                result.add_error(
                    &format!("product.{}.category_id", product.id),
                    "Category ID cannot be empty",
                );
            }
        }

        // Validate categories schema
        for category in &data.categories {
            if category.id.is_empty() {
                result.add_error(
                    &format!("category.{}.id", category.id),
                    "ID cannot be empty",
                );
            }
            if category.name.is_empty() {
                result.add_error(
                    &format!("category.{}.name", category.id),
                    "Name cannot be empty",
                );
            }
        }

        println!(
            "  Schema validation: {} errors, {} warnings",
            result.errors.len(),
            result.warnings.len()
        );

        // Merge with existing results
        let mut existing = ctx
            .get::<ValidationResult>("validation_result")
            .cloned()
            .unwrap_or_default();
        existing.merge(result);
        ctx.insert("validation_result", existing);

        Ok(StepOutput::next("business_validation"))
    }

    fn name(&self) -> StepName {
        StepName::new("SchemaValidation")
    }
}

// Step 3: Business rule validation
#[derive(Debug)]
struct BusinessValidationStep;

#[async_trait]
impl Step for BusinessValidationStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Running business rule validation...");

        let data =
            ctx.get::<ImportData>("import_data")
                .ok_or_else(|| WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Import data not found".to_string(),
                })?;

        let mut result = ValidationResult::default();

        for product in &data.products {
            // Price validation
            if product.price < 0.0 {
                result.add_error(
                    &format!("product.{}.price", product.id),
                    &format!("Price cannot be negative: {}", product.price),
                );
            } else if product.price == 0.0 {
                result.add_warning(
                    &format!("product.{}.price", product.id),
                    "Price is zero - is this intentional?",
                );
            }

            // Stock validation
            if product.stock < 0 {
                result.add_error(
                    &format!("product.{}.stock", product.id),
                    &format!("Stock cannot be negative: {}", product.stock),
                );
            } else if product.stock == 0 {
                result.add_warning(
                    &format!("product.{}.stock", product.id),
                    "Stock is zero - product will be unavailable",
                );
            }

            // Price range check
            if product.price > 10000.0 {
                result.add_warning(
                    &format!("product.{}.price", product.id),
                    &format!("Unusually high price: {}", product.price),
                );
            }
        }

        println!(
            "  Business validation: {} errors, {} warnings",
            result.errors.len(),
            result.warnings.len()
        );

        let mut existing = ctx
            .get::<ValidationResult>("validation_result")
            .cloned()
            .unwrap_or_default();
        existing.merge(result);
        ctx.insert("validation_result", existing);

        Ok(StepOutput::next("reference_validation"))
    }

    fn name(&self) -> StepName {
        StepName::new("BusinessValidation")
    }
}

// Step 4: Cross-reference validation
#[derive(Debug)]
struct ReferenceValidationStep;

#[async_trait]
impl Step for ReferenceValidationStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        println!("Running reference validation...");

        let data =
            ctx.get::<ImportData>("import_data")
                .ok_or_else(|| WorkflowError::StepError {
                    step_name: self.name(),
                    details: "Import data not found".to_string(),
                })?;

        let mut result = ValidationResult::default();

        // Build category lookup
        let category_ids: std::collections::HashSet<_> =
            data.categories.iter().map(|c| c.id.clone()).collect();

        // Check product -> category references
        for product in &data.products {
            if !category_ids.contains(&product.category_id) {
                result.add_error(
                    &format!("product.{}.category_id", product.id),
                    &format!("Category not found: {}", product.category_id),
                );
            }
        }

        // Check category -> parent references
        for category in &data.categories {
            if let Some(parent_id) = &category.parent_id {
                if !category_ids.contains(parent_id) {
                    result.add_error(
                        &format!("category.{}.parent_id", category.id),
                        &format!("Parent category not found: {}", parent_id),
                    );
                }
            }
        }

        println!(
            "  Reference validation: {} errors, {} warnings",
            result.errors.len(),
            result.warnings.len()
        );

        let mut existing = ctx
            .get::<ValidationResult>("validation_result")
            .cloned()
            .unwrap_or_default();
        existing.merge(result);
        ctx.insert("validation_result", existing);

        Ok(StepOutput::next("report"))
    }

    fn name(&self) -> StepName {
        StepName::new("ReferenceValidation")
    }
}

// Step 5: Generate report
#[derive(Debug)]
struct ReportStep;

#[async_trait]
impl Step for ReportStep {
    async fn execute(&self, ctx: &mut Context) -> Result<StepOutput, WorkflowError> {
        let mut result = ctx
            .get::<ValidationResult>("validation_result")
            .cloned()
            .unwrap_or_default();

        result.passed = result.errors.is_empty();

        println!("\n╔══════════════════════════════════════════════════════╗");
        println!("║           DATA VALIDATION REPORT                     ║");
        println!("╠══════════════════════════════════════════════════════╣");
        println!(
            "║ Status: {}                                      ║",
            if result.passed { "PASSED" } else { "FAILED" }
        );
        println!(
            "║ Errors:   {:>3}                                       ║",
            result.errors.len()
        );
        println!(
            "║ Warnings: {:>3}                                       ║",
            result.warnings.len()
        );
        println!("╠══════════════════════════════════════════════════════╣");

        if !result.errors.is_empty() {
            println!("║ ERRORS:                                              ║");
            for error in &result.errors {
                println!("║  [{}]                          ║", error.field);
                println!("║    {}  ║", error.message);
            }
        }

        if !result.warnings.is_empty() {
            println!("╠══════════════════════════════════════════════════════╣");
            println!("║ WARNINGS:                                            ║");
            for warning in &result.warnings {
                println!("║  [{}]                          ║", warning.field);
                println!("║    {}  ║", warning.message);
            }
        }

        println!("╚══════════════════════════════════════════════════════╝");

        ctx.insert("validation_result", result);

        Ok(StepOutput::done())
    }

    fn name(&self) -> StepName {
        StepName::new("Report")
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = Workflow::builder()
        .add_step("load", LoadDataStep)
        .add_step("schema_validation", SchemaValidationStep)
        .add_step("business_validation", BusinessValidationStep)
        .add_step("reference_validation", ReferenceValidationStep)
        .add_step("report", ReportStep)
        .start_with("load")
        .build()?;

    let mut ctx = Context::new();

    println!("=== Data Validation Pipeline ===\n");

    match workflow.execute(&mut ctx).await {
        Ok(()) => {
            let result = ctx.get::<ValidationResult>("validation_result");
            if let Some(r) = result {
                if r.passed {
                    println!("\nValidation passed! Data is ready for import.");
                } else {
                    println!("\nValidation failed! Please fix errors before import.");
                    std::process::exit(1);
                }
            }
        }
        Err(errors) => {
            for error in errors {
                eprintln!("Validation pipeline failed: {:?}", error);
            }
            std::process::exit(1);
        }
    }

    Ok(())
}
