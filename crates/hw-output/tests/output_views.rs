use hw_model::{
    Device, DeviceKind, DeviceProperties, PciInfo, ScanReport, SourceEvidence, SourceKind,
    SourceStatus,
};
use hw_output::{list_kinds, summary_text, table_text, to_flat_report, to_jsonl};

fn sample_report() -> ScanReport {
    let mut report = ScanReport::empty();
    report.devices.push(
        Device::new(
            "pci:0000:00:1f.3",
            DeviceKind::Pci,
            "Intel HD Audio",
            DeviceProperties::Pci(PciInfo {
                address: "0000:00:1f.3".to_string(),
                ..Default::default()
            }),
        )
        .with_source(SourceEvidence {
            source: "lspci -nn -k".to_string(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        }),
    );
    report
}

#[test]
fn flat_report_counts_devices_by_kind() {
    let flat = to_flat_report(&sample_report());
    assert_eq!(flat.summary.device_count, 1);
    assert_eq!(flat.summary.counts_by_kind.get("pci"), Some(&1));
}

#[test]
fn flat_report_preserves_json_contract_fields() {
    let json = serde_json::to_value(to_flat_report(&sample_report())).unwrap();
    assert_eq!(json["schema_version"], "qurbrix.hw.scan.v2");
    assert_eq!(json["status"], "complete");
    assert_eq!(json["summary"]["device_count"], 1);
    assert_eq!(json["summary"]["counts_by_kind"]["pci"], 1);
    assert_eq!(json["devices"][0]["id"], "pci:0000:00:1f.3");
    assert_eq!(json["devices"][0]["kind"], "pci");
    assert_eq!(json["devices"][0]["name"], "Intel HD Audio");
    assert_eq!(json["devices"][0]["properties"]["kind"], "pci");
    assert_eq!(
        json["devices"][0]["properties"]["data"]["address"],
        "0000:00:1f.3"
    );
    assert_eq!(json["devices"][0]["sources"][0]["source"], "lspci -nn -k");
}

#[test]
fn jsonl_outputs_one_device_line() {
    let text = to_jsonl(&sample_report()).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(text.contains("pci:0000:00:1f.3"));
}

#[test]
fn human_outputs_include_device_name() {
    assert!(summary_text(&sample_report()).contains("Devices: 1"));
    assert!(table_text(&sample_report(), None).contains("Intel HD Audio"));
    assert!(list_kinds().contains(&"other-pci".to_string()));
}
