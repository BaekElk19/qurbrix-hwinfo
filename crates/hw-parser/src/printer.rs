use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrinterStatusRecord {
    pub queue: String,
    pub accepting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrinterUriRecord {
    pub queue: String,
    pub device_uri: Option<String>,
}

pub fn parse_lpstat_a(input: &str) -> Vec<PrinterStatusRecord> {
    input
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let queue = parts.next()?.to_string();
            let state = parts.next()?;
            Some(PrinterStatusRecord {
                queue,
                accepting: state == "accepting",
            })
        })
        .collect()
}

pub fn parse_lpstat_v(input: &str) -> Vec<PrinterUriRecord> {
    input
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("device for ")?;
            let (queue, uri) = rest.split_once(':')?;
            Some(PrinterUriRecord {
                queue: queue.trim().to_string(),
                device_uri: Some(uri.trim().to_string()).filter(|value| !value.is_empty()),
            })
        })
        .collect()
}
