//! File Processing Pipeline.
//!
//! This example demonstrates batch file transformation:
//! 1. Scan directory for input files
//! 2. Parse and validate each file
//! 3. Transform data
//! 4. Write output files
//!
//! Use cases:
//! - Log file aggregation
//! - Data format conversion (JSON -> CSV, XML -> JSON)
//! - Batch report generation
//! - File migration tools

#![allow(dead_code)]

use std::collections::HashMap;
use tsumugi::prelude::*;

// Input file representation
#[derive(Debug, Clone)]
struct InputFile {
    path: String,
    size_bytes: u64,
    file_type: FileType,
}

#[derive(Debug, Clone, PartialEq)]
enum FileType {
    Json,
    Csv,
    Unknown,
}

// Parsed log entry
#[derive(Debug, Clone)]
struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
    source_file: String,
}

// Processing statistics
#[derive(Debug, Clone, Default)]
struct ProcessingStats {
    files_processed: usize,
    entries_parsed: usize,
    errors_count: usize,
    by_level: HashMap<String, usize>,
}

// Output report
#[derive(Debug, Clone)]
struct ProcessingReport {
    stats: ProcessingStats,
    output_file: String,
    entries: Vec<LogEntry>,
}

// Step 1: Scan input directory
#[derive(Debug)]
struct ScanDirectoryStep;

#[async_trait]
impl Step for ScanDirectoryStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Scanning input directory...");

        // In production, use std::fs::read_dir
        // let entries = std::fs::read_dir("./input")?;

        // Simulated file discovery
        let files = [
            InputFile {
                path: "./input/app-2024-01-01.json".to_string(),
                size_bytes: 1024,
                file_type: FileType::Json,
            },
            InputFile {
                path: "./input/app-2024-01-02.json".to_string(),
                size_bytes: 2048,
                file_type: FileType::Json,
            },
            InputFile {
                path: "./input/app-2024-01-03.json".to_string(),
                size_bytes: 1536,
                file_type: FileType::Json,
            },
            InputFile {
                path: "./input/legacy.csv".to_string(),
                size_bytes: 512,
                file_type: FileType::Csv,
            },
        ];

        let json_files: Vec<_> = files
            .iter()
            .filter(|f| f.file_type == FileType::Json)
            .cloned()
            .collect();

        println!("  Found {} files ({} JSON)", files.len(), json_files.len());

        ctx.insert("input_files", json_files);

        Ok(Next::step("parse"))
    }
}

// Step 2: Parse files
#[derive(Debug)]
struct ParseFilesStep;

#[async_trait]
impl Step for ParseFilesStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Parsing files...");

        let files = ctx.require::<Vec<InputFile>>("input_files")?.clone();

        let mut all_entries: Vec<LogEntry> = Vec::new();
        let mut stats = ProcessingStats::default();

        for file in &files {
            println!("  Processing: {}", file.path);

            // In production, read and parse actual files:
            // let content = std::fs::read_to_string(&file.path)?;
            // let entries: Vec<LogEntry> = serde_json::from_str(&content)?;

            // Simulated parsing
            let entries = simulate_parse_file(file);
            stats.files_processed += 1;
            stats.entries_parsed += entries.len();

            for entry in &entries {
                *stats.by_level.entry(entry.level.clone()).or_insert(0) += 1;
            }

            all_entries.extend(entries);
        }

        println!(
            "  Parsed {} entries from {} files",
            stats.entries_parsed, stats.files_processed
        );

        ctx.insert("log_entries", all_entries);
        ctx.insert("stats", stats);

        Ok(Next::step("filter"))
    }
}

// Simulate file parsing
fn simulate_parse_file(file: &InputFile) -> Vec<LogEntry> {
    let source = file.path.clone();
    vec![
        LogEntry {
            timestamp: "2024-01-15T10:00:00Z".to_string(),
            level: "INFO".to_string(),
            message: "Application started".to_string(),
            source_file: source.clone(),
        },
        LogEntry {
            timestamp: "2024-01-15T10:00:01Z".to_string(),
            level: "DEBUG".to_string(),
            message: "Loading configuration".to_string(),
            source_file: source.clone(),
        },
        LogEntry {
            timestamp: "2024-01-15T10:00:02Z".to_string(),
            level: "WARN".to_string(),
            message: "Deprecated API usage detected".to_string(),
            source_file: source.clone(),
        },
        LogEntry {
            timestamp: "2024-01-15T10:00:03Z".to_string(),
            level: "ERROR".to_string(),
            message: "Failed to connect to external service".to_string(),
            source_file: source,
        },
    ]
}

// Step 3: Filter and transform
#[derive(Debug)]
struct FilterEntriesStep;

#[async_trait]
impl Step for FilterEntriesStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Filtering entries...");

        let entries = ctx.require::<Vec<LogEntry>>("log_entries")?.clone();

        // Filter configuration (could come from context)
        let min_level = ctx
            .get::<String>("filter_level")
            .map(|s| s.as_str())
            .unwrap_or("WARN");

        let filtered: Vec<_> = entries
            .into_iter()
            .filter(|e| matches!(e.level.as_str(), "WARN" | "ERROR" | "FATAL"))
            .collect();

        println!(
            "  Filtered to {} entries (level >= {})",
            filtered.len(),
            min_level
        );

        ctx.insert("filtered_entries", filtered);

        Ok(Next::step("write_output"))
    }
}

// Step 4: Write output
#[derive(Debug)]
struct WriteOutputStep;

#[async_trait]
impl Step for WriteOutputStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        println!("Writing output...");

        let entries = ctx.require::<Vec<LogEntry>>("filtered_entries")?.clone();

        let stats = ctx.require::<ProcessingStats>("stats")?.clone();

        let output_file = "./output/aggregated_logs.csv".to_string();

        // In production, write to file:
        // let mut wtr = csv::Writer::from_path(&output_file)?;
        // for entry in &entries {
        //     wtr.write_record(&[&entry.timestamp, &entry.level, &entry.message])?;
        // }

        println!("  Output: {}", output_file);

        let report = ProcessingReport {
            stats,
            output_file,
            entries,
        };

        ctx.insert("report", report);

        Ok(Next::step("summary"))
    }
}

// Step 5: Print summary
#[derive(Debug)]
struct SummaryStep;

#[async_trait]
impl Step for SummaryStep {
    async fn run(&self, ctx: &mut Context) -> StepResult {
        let report = ctx.require::<ProcessingReport>("report")?;

        println!("\n┌─────────────────────────────────────────┐");
        println!("│     FILE PROCESSING SUMMARY             │");
        println!("├─────────────────────────────────────────┤");
        println!(
            "│ Files processed: {:>5}                  │",
            report.stats.files_processed
        );
        println!(
            "│ Entries parsed:  {:>5}                  │",
            report.stats.entries_parsed
        );
        println!(
            "│ Output entries:  {:>5}                  │",
            report.entries.len()
        );
        println!("├─────────────────────────────────────────┤");
        println!("│ Entries by level:                       │");
        for (level, count) in &report.stats.by_level {
            println!("│   {:<8}: {:>5}                        │", level, count);
        }
        println!("├─────────────────────────────────────────┤");
        println!("│ Output file: {}   │", report.output_file);
        println!("└─────────────────────────────────────────┘");

        // Preview output
        println!("\nOutput preview:");
        println!("timestamp,level,message");
        for entry in report.entries.iter().take(5) {
            println!("{},{},{}", entry.timestamp, entry.level, entry.message);
        }

        Ok(Next::Done)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let workflow = Workflow::builder()
        .add_step("scan", ScanDirectoryStep)
        .add_step("parse", ParseFilesStep)
        .add_step("filter", FilterEntriesStep)
        .add_step("write_output", WriteOutputStep)
        .add_step("summary", SummaryStep)
        .start_with("scan")
        .build()?;

    let mut ctx = Context::new();
    // Optional: set filter level
    ctx.insert("filter_level", "WARN".to_string());

    println!("=== File Processing Pipeline ===\n");

    match workflow.run(&mut ctx).await {
        Ok(_) => {
            println!("\nPipeline completed successfully!");
        }
        Err(err) => {
            eprintln!("Pipeline failed: {}", err);
            eprintln!("{}", err.report());
            std::process::exit(1);
        }
    }

    Ok(())
}
