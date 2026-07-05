use hw_model::{DeviceKind, SCHEMA_VERSION};

pub fn schema_version() -> &'static str {
    SCHEMA_VERSION
}

pub fn list_kinds() -> Vec<String> {
    DeviceKind::ALL
        .iter()
        .map(|kind| kind.to_string())
        .collect()
}
