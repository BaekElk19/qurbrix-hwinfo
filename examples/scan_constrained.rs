use hw_collect::{collect_scan_report_detailed, ScanExecutionOptions};
use hw_model::ScanConfig;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rounds = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10);
    println!("run,wall_ms,status,devices,warnings,cache_hits,peak_external");
    for run in 1..=rounds {
        let started = Instant::now();
        let collection = collect_scan_report_detailed(
            ScanConfig {
                timeout: Duration::from_secs(2),
                ..ScanConfig::default()
            },
            ScanExecutionOptions {
                max_external_commands: 1,
                ..ScanExecutionOptions::default()
            },
        )
        .await?;
        println!(
            "{run},{},{:?},{},{},{},{}",
            started.elapsed().as_millis(),
            collection.report.status,
            collection.report.devices.len(),
            collection.report.warnings.len(),
            collection.source_metrics.cache_hits,
            collection.source_metrics.peak_external_commands
        );
    }
    Ok(())
}
