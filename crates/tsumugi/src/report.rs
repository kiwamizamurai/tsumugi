//! Execution reports.

use std::fmt;
use std::time::Duration;
use tsumugi_core::StepName;

/// How a step execution ended.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StepStatus {
    /// The step succeeded and continued to the given step.
    Continued(StepName),
    /// The step succeeded and completed the workflow.
    Completed,
    /// The step failed after exhausting its retries.
    Failed,
}

/// Record of a single step execution, including all retry attempts.
#[derive(Debug, Clone)]
pub struct StepRecord {
    pub(crate) name: StepName,
    pub(crate) attempts: u32,
    pub(crate) duration: Duration,
    pub(crate) status: StepStatus,
}

impl StepRecord {
    /// Returns the name the step is registered under.
    pub fn name(&self) -> &StepName {
        &self.name
    }

    /// Returns the number of attempts, including the first one.
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Returns the number of retries (`attempts - 1`).
    pub fn retries(&self) -> u32 {
        self.attempts.saturating_sub(1)
    }

    /// Returns the total time spent on the step, including retry delays.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Returns how the step execution ended.
    pub fn status(&self) -> &StepStatus {
        &self.status
    }
}

/// Summary of a workflow execution.
///
/// Returned by [`Workflow::run`](crate::Workflow::run) on success, and
/// carried by [`ExecutionError`](crate::ExecutionError) on failure.
///
/// A step that runs more than once (for example in a loop) appears once per
/// execution.
#[derive(Debug, Clone, Default)]
pub struct ExecutionReport {
    pub(crate) steps: Vec<StepRecord>,
    pub(crate) duration: Duration,
}

impl ExecutionReport {
    /// Returns the executed steps in execution order.
    pub fn steps(&self) -> &[StepRecord] {
        &self.steps
    }

    /// Returns the names of the executed steps in execution order.
    pub fn path(&self) -> impl Iterator<Item = &StepName> {
        self.steps.iter().map(|record| &record.name)
    }

    /// Returns the total number of retries across all steps.
    pub fn total_retries(&self) -> u32 {
        self.steps.iter().map(StepRecord::retries).sum()
    }

    /// Returns the total workflow execution time.
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

impl fmt::Display for ExecutionReport {
    /// Formats the report as one line per executed step.
    ///
    /// ```text
    /// validate  1 attempt       0.1ms  -> charge
    /// charge    3 attempts    210.4ms  -> save
    /// save      1 attempt       1.2ms  done
    /// total: 211.7ms, 2 retries
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = self
            .steps
            .iter()
            .map(|record| record.name.as_str().len())
            .max()
            .unwrap_or(0);

        for record in &self.steps {
            let attempts = if record.attempts == 1 {
                "1 attempt".to_string()
            } else {
                format!("{} attempts", record.attempts)
            };
            let status = match &record.status {
                StepStatus::Continued(next) => format!("-> {}", next),
                StepStatus::Completed => "done".to_string(),
                StepStatus::Failed => "FAILED".to_string(),
            };
            writeln!(
                f,
                "{:<width$}  {:<11} {:>9}  {}",
                record.name.as_str(),
                attempts,
                format_duration(record.duration),
                status,
                width = width
            )?;
        }
        let retries = self.total_retries();
        write!(
            f,
            "total: {}, {} {}",
            format_duration(self.duration),
            retries,
            if retries == 1 { "retry" } else { "retries" }
        )
    }
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs() > 0 {
        format!("{:.2}s", duration.as_secs_f64())
    } else {
        format!("{:.1}ms", duration.as_secs_f64() * 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(name: &str, attempts: u32, status: StepStatus) -> StepRecord {
        StepRecord {
            name: StepName::new(name),
            attempts,
            duration: Duration::from_millis(5),
            status,
        }
    }

    #[test]
    fn test_report_accessors() {
        let report = ExecutionReport {
            steps: vec![
                record("fetch", 3, StepStatus::Continued(StepName::new("save"))),
                record("save", 1, StepStatus::Completed),
            ],
            duration: Duration::from_millis(10),
        };

        let path: Vec<&str> = report.path().map(StepName::as_str).collect();
        assert_eq!(path, ["fetch", "save"]);
        assert_eq!(report.total_retries(), 2);
        assert_eq!(report.steps()[0].retries(), 2);
    }

    #[test]
    fn test_report_display() {
        let report = ExecutionReport {
            steps: vec![
                record("fetch", 3, StepStatus::Continued(StepName::new("save"))),
                record("save", 1, StepStatus::Failed),
            ],
            duration: Duration::from_millis(10),
        };

        assert_eq!(
            report.to_string(),
            "fetch  3 attempts      5.0ms  -> save\n\
             save   1 attempt       5.0ms  FAILED\n\
             total: 10.0ms, 2 retries"
        );
    }
}
