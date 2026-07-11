use serde::Serialize;

pub const SCHEMA_VERSION: &str = "qurbrix.hw.bindid.v1";
pub const ALGORITHM: &str = "qurbrix-hw-bindid-sha1-hex16-v1";

pub const REQUIRED_KINDS: &[&str] = &["system", "motherboard", "memory", "storage", "network"];
pub const OPTIONAL_KINDS: &[&str] = &["gpu"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindIdStatus {
    Complete,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BindIdReport {
    pub schema_version: String,
    pub algorithm: String,
    pub status: BindIdStatus,
    pub value: Option<String>,
    pub required_kinds: Vec<String>,
    pub optional_kinds: Vec<String>,
    pub covered_kinds: Vec<String>,
    pub missing_required_kinds: Vec<String>,
    pub missing_optional_kinds: Vec<String>,
    pub component_keys: Vec<String>,
    pub warnings: Vec<String>,
}

impl BindIdReport {
    pub fn from_parts(component_keys: Vec<String>, warnings: Vec<String>) -> Self {
        let covered_kinds = covered_kinds(&component_keys);
        let missing_required_kinds = missing_kinds(REQUIRED_KINDS, &covered_kinds);
        let missing_optional_kinds = missing_kinds(OPTIONAL_KINDS, &covered_kinds);
        let status = if missing_required_kinds.is_empty() {
            BindIdStatus::Complete
        } else {
            BindIdStatus::Failed
        };
        let value =
            (status == BindIdStatus::Complete).then(|| crate::key::bindid_value(&component_keys));

        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            algorithm: ALGORITHM.to_string(),
            status,
            value,
            required_kinds: REQUIRED_KINDS
                .iter()
                .map(|kind| (*kind).to_string())
                .collect(),
            optional_kinds: OPTIONAL_KINDS
                .iter()
                .map(|kind| (*kind).to_string())
                .collect(),
            covered_kinds,
            missing_required_kinds,
            missing_optional_kinds,
            component_keys: sorted(component_keys),
            warnings,
        }
    }
}

fn covered_kinds(component_keys: &[String]) -> Vec<String> {
    let mut kinds = component_keys
        .iter()
        .filter_map(|key| key.split_once(':').map(|(kind, _)| kind.to_string()))
        .collect::<Vec<_>>();
    kinds.sort();
    kinds.dedup();
    kinds
}

fn missing_kinds(expected: &[&str], covered: &[String]) -> Vec<String> {
    expected
        .iter()
        .filter(|kind| !covered.iter().any(|covered_kind| covered_kind == *kind))
        .map(|kind| (*kind).to_string())
        .collect()
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}
