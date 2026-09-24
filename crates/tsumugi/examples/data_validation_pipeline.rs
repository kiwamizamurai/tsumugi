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

use std::collections::HashSet;
use tsumugi::prelude::*;

/// The state shared by all steps of the pipeline.
#[derive(Default)]
struct ValidationState {
    /// Set by the load step.
    data: Option<ImportData>,
    /// Accumulates the findings of every validation step.
    result: ValidationResult,
}

// Input data to validate
struct ImportData {
    products: Vec<Product>,
    categories: Vec<Category>,
}

struct Product {
    id: String,
    name: String,
    price: f64,
    category_id: String,
    stock: i32,
}

struct Category {
    id: String,
    name: String,
    parent_id: Option<String>,
}

// Validation result
struct ValidationError {
    field: String,
    message: String,
    // Not read by this demo, which reports errors and warnings separately.
    #[allow(dead_code)]
    severity: Severity,
}

enum Severity {
    Error,
    Warning,
}

#[derive(Default)]
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
struct LoadDataStep;

#[async_trait]
impl Step<ValidationState> for LoadDataStep {
    async fn run(&self, state: &mut ValidationState) -> StepResult {
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

        state.data = Some(data);

        Ok(Next::step("schema_validation"))
    }
}

// Step 2: Schema validation
struct SchemaValidationStep;

#[async_trait]
impl Step<ValidationState> for SchemaValidationStep {
    async fn run(&self, state: &mut ValidationState) -> StepResult {
        println!("Running schema validation...");

        let data = state.data.as_ref().ok_or("import data not loaded")?;

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
        state.result.merge(result);

        Ok(Next::step("business_validation"))
    }
}

// Step 3: Business rule validation
struct BusinessValidationStep;

#[async_trait]
impl Step<ValidationState> for BusinessValidationStep {
    async fn run(&self, state: &mut ValidationState) -> StepResult {
        println!("Running business rule validation...");

        let data = state.data.as_ref().ok_or("import data not loaded")?;

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

        state.result.merge(result);

        Ok(Next::step("reference_validation"))
    }
}

// Step 4: Cross-reference validation
struct ReferenceValidationStep;

#[async_trait]
impl Step<ValidationState> for ReferenceValidationStep {
    async fn run(&self, state: &mut ValidationState) -> StepResult {
        println!("Running reference validation...");

        let data = state.data.as_ref().ok_or("import data not loaded")?;

        let mut result = ValidationResult::default();

        // Build category lookup
        let category_ids: HashSet<&str> = data.categories.iter().map(|c| c.id.as_str()).collect();

        // Check product -> category references
        for product in &data.products {
            if !category_ids.contains(product.category_id.as_str()) {
                result.add_error(
                    &format!("product.{}.category_id", product.id),
                    &format!("Category not found: {}", product.category_id),
                );
            }
        }

        // Check category -> parent references
        for category in &data.categories {
            if let Some(parent_id) = &category.parent_id {
                if !category_ids.contains(parent_id.as_str()) {
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

        state.result.merge(result);

        Ok(Next::step("report"))
    }
}

// Step 5: Generate report
struct ReportStep;

#[async_trait]
impl Step<ValidationState> for ReportStep {
    async fn run(&self, state: &mut ValidationState) -> StepResult {
        let result = &mut state.result;
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

        Ok(Next::Done)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = WorkflowBuilder::<ValidationState>::new()
        .add_step("load", LoadDataStep)
        .then(["schema_validation"])
        .add_step("schema_validation", SchemaValidationStep)
        .then(["business_validation"])
        .add_step("business_validation", BusinessValidationStep)
        .then(["reference_validation"])
        .add_step("reference_validation", ReferenceValidationStep)
        .then(["report"])
        .add_step("report", ReportStep)
        .terminal()
        .build()?;

    let mut state = ValidationState::default();

    println!("=== Data Validation Pipeline ===\n");

    match workflow.run(&mut state).await {
        Ok(_) => {
            if state.result.passed {
                println!("\nValidation passed! Data is ready for import.");
            } else {
                println!("\nValidation failed! Please fix errors before import.");
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("Validation pipeline failed: {}", err);
            eprintln!("{}", err.report());
            std::process::exit(1);
        }
    }

    Ok(())
}
