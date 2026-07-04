pub async fn collect_scan_report() -> anyhow::Result<hw_model::ScanReport> {
    Ok(hw_model::ScanReport::empty())
}


pub async fn collect_system_info() -> anyhow::Result<hw_model::SystemInfo> {
    Ok(hw_model::SystemInfo::empty())
}

pub async fn refresh_system_info() -> anyhow::Result<hw_model::SystemInfo> {
    collect_system_info().await
}
