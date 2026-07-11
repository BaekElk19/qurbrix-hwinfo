use crate::model::BindIdReport;
use anyhow::Result;
use hw_source::{RealSourceRunner, SourceRunner};
use std::time::Duration;

pub async fn collect_bindid_report(timeout: Duration) -> Result<BindIdReport> {
    let runner = RealSourceRunner;
    collect_bindid_report_with_runner(&runner, timeout).await
}

pub async fn collect_bindid_report_with_runner(
    _runner: &dyn SourceRunner,
    _timeout: Duration,
) -> Result<BindIdReport> {
    Ok(BindIdReport::from_parts(Vec::new(), Vec::new()))
}
