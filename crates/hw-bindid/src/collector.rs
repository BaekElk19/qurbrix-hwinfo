use crate::model::{BindIdReport, BindIdStatus, ALGORITHM, SCHEMA_VERSION};
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
    Ok(BindIdReport {
        schema_version: SCHEMA_VERSION.to_string(),
        algorithm: ALGORITHM.to_string(),
        status: BindIdStatus::Failed,
        value: None,
        required_kinds: vec![
            "system".to_string(),
            "motherboard".to_string(),
            "memory".to_string(),
            "storage".to_string(),
            "network".to_string(),
        ],
        optional_kinds: vec!["gpu".to_string()],
        covered_kinds: Vec::new(),
        missing_required_kinds: vec![
            "system".to_string(),
            "motherboard".to_string(),
            "memory".to_string(),
            "storage".to_string(),
            "network".to_string(),
        ],
        missing_optional_kinds: vec!["gpu".to_string()],
        component_keys: Vec::new(),
        warnings: Vec::new(),
    })
}
