use hw_inventory::{quick_probe, QuickProbeConfig};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rounds = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10);
    println!("run,wall_ms,core_complete,identity_records,warnings");
    for run in 1..=rounds {
        let started = Instant::now();
        let report = quick_probe(QuickProbeConfig {
            timeout: Duration::from_secs(2),
        })
        .await?;
        println!(
            "{run},{},{},{},{}",
            started.elapsed().as_millis(),
            report.coverage.core_complete(),
            report.identity_records.len(),
            report.warnings.len()
        );
    }
    Ok(())
}
