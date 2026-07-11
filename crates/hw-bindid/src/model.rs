pub const SCHEMA_VERSION: &str = "qurbrix.hw.bindid.v1";
pub const ALGORITHM: &str = "qurbrix-hw-bindid-sha1-hex16-v1";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindIdStatus {
    Complete,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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
