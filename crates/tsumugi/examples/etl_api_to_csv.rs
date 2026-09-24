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

use std::collections::BTreeMap;
use tsumugi::prelude::*;

// Simulated API response data
struct ApiResponse {
    users: Vec<User>,
    // Part of the simulated payload, not used by this pipeline.
    #[allow(dead_code)]
    fetched_at: String,
}

struct User {
    id: u64,
    name: String,
    email: String,
    department: String,
    active: bool,
}

// Transformed data ready for CSV
struct CsvRecord {
    id: u64,
    name: String,
    email: String,
    department: String,
    status: String,
}

struct CsvOutput {
    headers: Vec<String>,
    records: Vec<CsvRecord>,
    filename: String,
}

/// The state shared by the pipeline steps. Each field is filled in by one
/// step and read by a later one.
#[derive(Default)]
struct EtlState {
    response: Option<ApiResponse>,
    records: Option<Vec<CsvRecord>>,
    department_stats: Option<BTreeMap<String, usize>>,
    output: Option<CsvOutput>,
}

// Step 1: Fetch data from REST API
struct FetchApiDataStep;

#[async_trait]
impl Step<EtlState> for FetchApiDataStep {
    async fn run(&self, state: &mut EtlState) -> StepResult {
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
        state.response = Some(response);

        Ok(Next::step("transform"))
    }
}

// Step 2: Transform data
struct TransformDataStep;

#[async_trait]
impl Step<EtlState> for TransformDataStep {
    async fn run(&self, state: &mut EtlState) -> StepResult {
        println!("Transforming data...");

        let response = state
            .response
            .as_ref()
            .ok_or("API response has not been fetched")?;

        // Transform: map users to CSV records
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
        let mut dept_counts: BTreeMap<String, usize> = BTreeMap::new();
        for record in &records {
            *dept_counts.entry(record.department.clone()).or_insert(0) += 1;
        }
        state.department_stats = Some(dept_counts);
        state.records = Some(records);

        Ok(Next::step("generate_csv"))
    }
}

// Step 3: Generate CSV output
struct GenerateCsvStep;

#[async_trait]
impl Step<EtlState> for GenerateCsvStep {
    async fn run(&self, state: &mut EtlState) -> StepResult {
        println!("Generating CSV...");

        let records = state
            .records
            .take()
            .ok_or("CSV records have not been transformed")?;

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
        state.output = Some(output);

        Ok(Next::step("summary"))
    }
}

// Step 4: Print summary
struct SummaryStep;

#[async_trait]
impl Step<EtlState> for SummaryStep {
    async fn run(&self, state: &mut EtlState) -> StepResult {
        println!("Generating summary...");

        let output = state
            .output
            .as_ref()
            .ok_or("CSV output has not been generated")?;
        let stats = state
            .department_stats
            .as_ref()
            .ok_or("department statistics have not been computed")?;

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

    let workflow = WorkflowBuilder::<EtlState>::new()
        .add_step("fetch", FetchApiDataStep)
        .then(["transform"])
        .add_step("transform", TransformDataStep)
        .then(["generate_csv"])
        .add_step("generate_csv", GenerateCsvStep)
        .then(["summary"])
        .add_step("summary", SummaryStep)
        .terminal()
        .build()?;

    let mut state = EtlState::default();

    println!("=== ETL Pipeline: REST API to CSV ===\n");

    match workflow.run(&mut state).await {
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
