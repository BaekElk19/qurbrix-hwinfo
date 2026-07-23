use hw_model::ScanConfig;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rounds = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10);
    println!("run,wall_ms,status,devices,warnings");
    for run in 1..=rounds {
        let started = Instant::now();
        let report = hw_collect::collect_scan_report(ScanConfig {
            timeout: Duration::from_secs(2),
            ..ScanConfig::default()
        })
        .await?;
        println!(
            "{run},{},{:?},{},{}",
            started.elapsed().as_millis(),
            report.status,
            report.devices.len(),
            report.warnings.len()
        );
    }
    Ok(())
}
