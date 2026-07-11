use hw_bindid::{BindIdReport, BindIdStatus, ALGORITHM, SCHEMA_VERSION};

const SYSTEM_KEY: &str = "system:manufacturer=GEIT|product=UT6619-FC2";
const MOTHERBOARD_KEY: &str = "motherboard:product=Board|serial=BOARD123";
const MEMORY_KEY: &str = "memory:product=DDR4|serial=MEM123";
const STORAGE_KEY: &str = "storage:model=Disk|serial=DISK123";
const NETWORK_KEY: &str = "network:mac=aa:bb:cc:dd:ee:ff";
const GPU_KEY: &str = "gpu:model=UHD Graphics 770";

fn base_keys() -> Vec<String> {
    [
        SYSTEM_KEY,
        MOTHERBOARD_KEY,
        MEMORY_KEY,
        STORAGE_KEY,
        NETWORK_KEY,
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[test]
fn complete_report_includes_bindid_value_and_preserves_warnings() {
    let warnings = vec!["normalized storage serial".to_string()];

    let report = BindIdReport::from_parts(base_keys(), warnings.clone());

    assert_eq!(report.schema_version, SCHEMA_VERSION);
    assert_eq!(report.algorithm, ALGORITHM);
    assert_eq!(report.status, BindIdStatus::Complete);
    assert_eq!(report.missing_required_kinds, Vec::<String>::new());
    assert_eq!(report.missing_optional_kinds, vec!["gpu".to_string()]);
    assert_eq!(report.warnings, warnings);

    let value = report.value.expect("complete report should have a bindid");
    assert_eq!(value.len(), 16);
    assert!(value.chars().all(|ch| matches!(ch, '0'..='9' | 'a'..='f')));
}

#[test]
fn missing_required_network_fails_report_without_value() {
    let keys = [SYSTEM_KEY, MOTHERBOARD_KEY, MEMORY_KEY, STORAGE_KEY]
        .into_iter()
        .map(str::to_string)
        .collect();

    let report = BindIdReport::from_parts(keys, Vec::new());

    assert_eq!(report.status, BindIdStatus::Failed);
    assert_eq!(report.value, None);
    assert_eq!(report.missing_required_kinds, vec!["network".to_string()]);
}

#[test]
fn sorts_component_keys_and_dedups_covered_kinds() {
    let keys = vec![
        SYSTEM_KEY.to_string(),
        STORAGE_KEY.to_string(),
        NETWORK_KEY.to_string(),
        MOTHERBOARD_KEY.to_string(),
        MEMORY_KEY.to_string(),
        MEMORY_KEY.to_string(),
    ];

    let report = BindIdReport::from_parts(keys, Vec::new());

    assert_eq!(
        report.component_keys,
        vec![
            MEMORY_KEY.to_string(),
            MEMORY_KEY.to_string(),
            MOTHERBOARD_KEY.to_string(),
            NETWORK_KEY.to_string(),
            STORAGE_KEY.to_string(),
            SYSTEM_KEY.to_string(),
        ]
    );
    assert_eq!(
        report.covered_kinds,
        vec![
            "memory".to_string(),
            "motherboard".to_string(),
            "network".to_string(),
            "storage".to_string(),
            "system".to_string(),
        ]
    );
}

#[test]
fn optional_gpu_affects_missing_optional_without_affecting_completeness() {
    let without_gpu = BindIdReport::from_parts(base_keys(), Vec::new());
    assert_eq!(without_gpu.status, BindIdStatus::Complete);
    assert_eq!(without_gpu.missing_optional_kinds, vec!["gpu".to_string()]);

    let mut with_gpu_keys = base_keys();
    with_gpu_keys.push(GPU_KEY.to_string());
    let with_gpu = BindIdReport::from_parts(with_gpu_keys, Vec::new());

    assert_eq!(with_gpu.status, BindIdStatus::Complete);
    assert!(with_gpu.missing_optional_kinds.is_empty());
}
