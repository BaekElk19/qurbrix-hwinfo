use hw_collect::{collect_scan_report_detailed, ScanExecutionOptions};
use hw_model::ScanConfig;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rounds = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10);
    println!("run,serial_ms,parallel_ms,improvement_percent,status_equal,device_count_equal,warning_count_equal,cache_hits,peak_external");
    for run in 1..=rounds {
        let config = ScanConfig {
            timeout: Duration::from_secs(2),
            ..ScanConfig::default()
        };
        let started = Instant::now();
        let serial = collect_scan_report_detailed(
            config.clone(),
            ScanExecutionOptions::serial_baseline(),
        )
        .await?;
        let serial_ms = started.elapsed().as_millis();
        let started = Instant::now();
        let parallel =
            collect_scan_report_detailed(config, ScanExecutionOptions::default()).await?;
        let parallel_ms = started.elapsed().as_millis();
        let improvement = if serial_ms == 0 {
            0.0
        } else {
            100.0 * (serial_ms.saturating_sub(parallel_ms)) as f64 / serial_ms as f64
        };
        println!(
            "{run},{serial_ms},{parallel_ms},{improvement:.2},{},{},{},{},{}",
            serial.report.status == parallel.report.status,
            serial.report.devices.len() == parallel.report.devices.len(),
            serial.report.warnings.len() == parallel.report.warnings.len(),
            parallel.source_metrics.cache_hits,
            parallel.source_metrics.peak_external_commands
        );
    }
    Ok(())
}
