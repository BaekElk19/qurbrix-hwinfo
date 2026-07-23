use hw_model::ScanReport;
use hw_source::SourceMetrics;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanExecutionOptions {
    pub max_external_commands: usize,
    pub global_deadline: Option<Duration>,
    pub cache_sources: bool,
    pub parallel_probes: bool,
}

impl Default for ScanExecutionOptions {
    fn default() -> Self {
        Self {
            max_external_commands: 4,
            global_deadline: Some(Duration::from_secs(120)),
            cache_sources: true,
            parallel_probes: true,
        }
    }
}

impl ScanExecutionOptions {
    pub fn serial_baseline() -> Self {
        Self {
            max_external_commands: 1,
            global_deadline: Some(Duration::from_secs(120)),
            cache_sources: false,
            parallel_probes: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeMetrics {
    pub name: String,
    pub duration_micros: u64,
    pub device_count: usize,
    pub warning_count: usize,
    pub timed_out: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScanCollection {
    pub report: ScanReport,
    pub probe_metrics: Vec<ProbeMetrics>,
    pub source_metrics: SourceMetrics,
    pub deadline_exceeded: bool,
}
