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

// Some fields and variants exist only to make the simulated data realistic.
#![allow(dead_code)]

use std::collections::BTreeMap;
use tsumugi::prelude::*;

// Input file representation
struct InputFile {
    path: String,
    size_bytes: u64,
    file_type: FileType,
}

#[derive(PartialEq)]
enum FileType {
    Json,
    Csv,
    Unknown,
}

// Parsed log entry
struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
    source_file: String,
}

// Processing statistics
#[derive(Default)]
struct ProcessingStats {
    files_processed: usize,
    entries_parsed: usize,
    errors_count: usize,
    by_level: BTreeMap<String, usize>,
}

// Output report
struct ProcessingReport {
    stats: ProcessingStats,
    output_file: String,
    entries: Vec<LogEntry>,
}

/// The state shared by the pipeline steps. `filter_level` is configured up
/// front; the other fields are filled in by the steps as the pipeline runs.
struct PipelineState {
    filter_level: String,
    input_files: Option<Vec<InputFile>>,
    log_entries: Option<Vec<LogEntry>>,
    stats: Option<ProcessingStats>,
    filtered_entries: Option<Vec<LogEntry>>,
    report: Option<ProcessingReport>,
}

impl PipelineState {
    fn new(filter_level: impl Into<String>) -> Self {
        Self {
            filter_level: filter_level.into(),
            input_files: None,
            log_entries: None,
            stats: None,
            filtered_entries: None,
            report: None,
        }
    }
}

// Step 1: Scan input directory
struct ScanDirectoryStep;

#[async_trait]
impl Step<PipelineState> for ScanDirectoryStep {
    async fn run(&self, state: &mut PipelineState) -> StepResult {
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

        let total = files.len();
        let json_files: Vec<_> = files
            .into_iter()
            .filter(|f| f.file_type == FileType::Json)
            .collect();

        println!("  Found {} files ({} JSON)", total, json_files.len());

        state.input_files = Some(json_files);

        Ok(Next::step("parse"))
    }
}

// Step 2: Parse files
struct ParseFilesStep;

#[async_trait]
impl Step<PipelineState> for ParseFilesStep {
    async fn run(&self, state: &mut PipelineState) -> StepResult {
        println!("Parsing files...");

        let files = state
            .input_files
            .as_ref()
            .ok_or("input files have not been scanned")?;

        let mut all_entries: Vec<LogEntry> = Vec::new();
        let mut stats = ProcessingStats::default();

        for file in files {
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

        state.log_entries = Some(all_entries);
        state.stats = Some(stats);

        Ok(Next::step("filter"))
    }
}

// Simulate file parsing
fn simulate_parse_file(file: &InputFile) -> Vec<LogEntry> {
    let source = &file.path;
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
            source_file: source.clone(),
        },
    ]
}

// Step 3: Filter and transform
struct FilterEntriesStep;

#[async_trait]
impl Step<PipelineState> for FilterEntriesStep {
    async fn run(&self, state: &mut PipelineState) -> StepResult {
        println!("Filtering entries...");

        let entries = state
            .log_entries
            .take()
            .ok_or("log entries have not been parsed")?;

        let filtered: Vec<_> = entries
            .into_iter()
            .filter(|e| matches!(e.level.as_str(), "WARN" | "ERROR" | "FATAL"))
            .collect();

        println!(
            "  Filtered to {} entries (level >= {})",
            filtered.len(),
            state.filter_level
        );

        state.filtered_entries = Some(filtered);

        Ok(Next::step("write_output"))
    }
}

// Step 4: Write output
struct WriteOutputStep;

#[async_trait]
impl Step<PipelineState> for WriteOutputStep {
    async fn run(&self, state: &mut PipelineState) -> StepResult {
        println!("Writing output...");

        let entries = state
            .filtered_entries
            .take()
            .ok_or("log entries have not been filtered")?;
        let stats = state
            .stats
            .take()
            .ok_or("processing statistics are missing")?;

        let output_file = "./output/aggregated_logs.csv".to_string();

        // In production, write to file:
        // let mut wtr = csv::Writer::from_path(&output_file)?;
        // for entry in &entries {
        //     wtr.write_record(&[&entry.timestamp, &entry.level, &entry.message])?;
        // }

        println!("  Output: {}", output_file);

        state.report = Some(ProcessingReport {
            stats,
            output_file,
            entries,
        });

        Ok(Next::step("summary"))
    }
}

// Step 5: Print summary
struct SummaryStep;

#[async_trait]
impl Step<PipelineState> for SummaryStep {
    async fn run(&self, state: &mut PipelineState) -> StepResult {
        let report = state
            .report
            .as_ref()
            .ok_or("processing report has not been written")?;

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

    let workflow = WorkflowBuilder::<PipelineState>::new()
        .add_step("scan", ScanDirectoryStep)
        .then(["parse"])
        .add_step("parse", ParseFilesStep)
        .then(["filter"])
        .add_step("filter", FilterEntriesStep)
        .then(["write_output"])
        .add_step("write_output", WriteOutputStep)
        .then(["summary"])
        .add_step("summary", SummaryStep)
        .terminal()
        .build()?;

    let mut state = PipelineState::new("WARN");

    println!("=== File Processing Pipeline ===\n");

    match workflow.run(&mut state).await {
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
