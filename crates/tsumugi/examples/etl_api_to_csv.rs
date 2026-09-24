//! ETL Pipeline: REST API to CSV file generation.
//!
//! This example demonstrates:
//! 1. Fetching data from a REST API
//! 2. Transforming the JSON response
//! 3. Writing to a CSV file
//!
//! ## GitHub Actions Example
//!
//! ```yaml
//! on:
//!   schedule:
//!     - cron: '0 0 * * *'  # Daily at midnight UTC
//! jobs:
//!   etl:
//!     runs-on: ubuntu-latest
//!     steps:
//!       - uses: actions/checkout@v4
//!       - run: cargo run --example etl_api_to_csv
//!       - uses: actions/upload-artifact@v4
//!         with:
//!           name: daily-report
//!           path: output/*.csv
//! ```

#![allow(dead_code)]

use std::collections::HashMap;
use tsumugi::prelude::*;

// Simulated API response data
#[derive(Debug, Clone)]
struct ApiResponse {
    users: Vec<User>,
    fetched_at: String,
}

#[derive(Debug, Clone)]
struct User {
    id: u64,
    name: String,
    email: String,
    department: String,
    active: bool,
}

// Transformed data ready for CSV
#[derive(Debug, Clone)]
struct CsvRecord {
    id: u64,
    name: String,
    email: String,
    department: String,
    status: String,
}

#[derive(Debug, Clone)]
struct CsvOutput {
    headers: Vec<String>,
    records: Vec<CsvRecord>,
    filename: String,
}

// Step 1: Fetch data from REST API
#[derive(Debug)]
struct FetchApiDataStep;

#[async_trait]
impl Step for FetchApiDataStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Fetching data from REST API...");

        // In production, use reqwest or similar:
        // let response = reqwest::get("https://api.example.com/users")
        //     .await?
        //     .json::<ApiResponse>()
        //     .await?;

        // Simulated API response
        let response = ApiResponse {
            users: vec![
                User {
                    id: 1,
                    name: "Alice Johnson".to_string(),
                    email: "alice@example.com".to_string(),
                    department: "Engineering".to_string(),
                    active: true,
                },
                User {
                    id: 2,
                    name: "Bob Smith".to_string(),
                    email: "bob@example.com".to_string(),
                    department: "Marketing".to_string(),
                    active: true,
                },
                User {
                    id: 3,
                    name: "Carol White".to_string(),
                    email: "carol@example.com".to_string(),
                    department: "Engineering".to_string(),
                    active: false,
                },
                User {
                    id: 4,
                    name: "David Brown".to_string(),
                    email: "david@example.com".to_string(),
                    department: "Sales".to_string(),
                    active: true,
                },
            ],
            fetched_at: "2024-01-15T00:00:00Z".to_string(),
        };

        println!("  Fetched {} users", response.users.len());
        ctx.insert("api_response", response);

        Ok(Next::step("transform"))
    }
}

// Step 2: Transform data
#[derive(Debug)]
struct TransformDataStep;

#[async_trait]
impl Step for TransformDataStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Transforming data...");

        let response = ctx.require::<ApiResponse>("api_response")?;

        // Transform: filter active users and map to CSV records
        let records: Vec<CsvRecord> = response
            .users
            .iter()
            .map(|user| CsvRecord {
                id: user.id,
                name: user.name.clone(),
                email: user.email.clone(),
                department: user.department.clone(),
                status: if user.active { "Active" } else { "Inactive" }.to_string(),
            })
            .collect();

        println!("  Transformed {} records", records.len());

        // Group by department for statistics
        let mut dept_counts: HashMap<String, usize> = HashMap::new();
        for record in &records {
            *dept_counts.entry(record.department.clone()).or_insert(0) += 1;
        }
        ctx.insert("department_stats", dept_counts);
        ctx.insert("csv_records", records);

        Ok(Next::step("generate_csv"))
    }
}

// Step 3: Generate CSV output
#[derive(Debug)]
struct GenerateCsvStep;

#[async_trait]
impl Step for GenerateCsvStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Generating CSV...");

        let records = ctx.require::<Vec<CsvRecord>>("csv_records")?.clone();

        let output = CsvOutput {
            headers: vec![
                "ID".to_string(),
                "Name".to_string(),
                "Email".to_string(),
                "Department".to_string(),
                "Status".to_string(),
            ],
            records,
            filename: format!("users_{}.csv", chrono_date()),
        };

        // In production, write to file:
        // std::fs::create_dir_all("output")?;
        // let mut wtr = csv::Writer::from_path(&output.filename)?;
        // wtr.write_record(&output.headers)?;
        // for record in &output.records {
        //     wtr.write_record(&[...])? ;
        // }

        println!("  Generated: {}", output.filename);
        ctx.insert("csv_output", output);

        Ok(Next::step("summary"))
    }
}

// Step 4: Print summary
#[derive(Debug)]
struct SummaryStep;

#[async_trait]
impl Step for SummaryStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Generating summary...");

        let output = ctx.require::<CsvOutput>("csv_output")?;

        let stats = ctx.require::<HashMap<String, usize>>("department_stats")?;

        println!("\n=== ETL Summary ===");
        println!("Output file: {}", output.filename);
        println!("Total records: {}", output.records.len());
        println!("Department breakdown:");
        for (dept, count) in stats {
            println!("  - {}: {}", dept, count);
        }

        // Simulate CSV content
        println!("\nCSV Preview:");
        println!("{}", output.headers.join(","));
        for record in output.records.iter().take(3) {
            println!(
                "{},{},{},{},{}",
                record.id, record.name, record.email, record.department, record.status
            );
        }
        if output.records.len() > 3 {
            println!("... and {} more rows", output.records.len() - 3);
        }

        Ok(Next::Done)
    }
}

// Helper to get current date (simplified)
fn chrono_date() -> String {
    "2024-01-15".to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = Workflow::builder()
        .add_step("fetch", FetchApiDataStep)
        .add_step("transform", TransformDataStep)
        .add_step("generate_csv", GenerateCsvStep)
        .add_step("summary", SummaryStep)
        .start_with("fetch")
        .build()?;

    let mut ctx = Context::new();

    println!("=== ETL Pipeline: REST API to CSV ===\n");

    match workflow.run(&mut ctx).await {
        Ok(_) => {
            println!("\nETL pipeline completed successfully!");
        }
        Err(err) => {
            eprintln!("ETL pipeline failed: {}", err);
            eprintln!("{}", err.report());
            std::process::exit(1);
        }
    }

    Ok(())
}
