use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CpuRecord {
    pub architecture: Option<String>,
    pub threads: Option<u32>,
    pub model_name: Option<String>,
    pub vendor: Option<String>,
    pub cores_per_socket: Option<u32>,
    pub sockets: Option<u32>,
}

pub fn parse_lscpu(input: &str) -> CpuRecord {
    let mut record = CpuRecord::default();
    for line in input.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Architecture" => record.architecture = Some(value.to_string()),
            "CPU(s)" => record.threads = value.parse().ok(),
            "Model name" => record.model_name = Some(value.to_string()),
            "Vendor ID" => record.vendor = Some(value.to_string()),
            "Core(s) per socket" => record.cores_per_socket = value.parse().ok(),
            "Socket(s)" => record.sockets = value.parse().ok(),
            _ => {}
        }
    }
    record
}
