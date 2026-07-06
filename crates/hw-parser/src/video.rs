use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VideoDeviceRecord {
    pub name: String,
    pub bus_hint: Option<String>,
    pub nodes: Vec<String>,
}

pub fn parse_v4l2_list_devices(input: &str) -> Vec<VideoDeviceRecord> {
    let mut records = Vec::new();
    let mut current: Option<VideoDeviceRecord> = None;
    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with('\t') && line.ends_with(':') {
            if let Some(record) = current.take() {
                records.push(record);
            }
            let header = line.trim_end_matches(':');
            let (name, bus_hint) = match header.rsplit_once('(') {
                Some((name, bus)) => (
                    name.trim().to_string(),
                    Some(bus.trim_end_matches(')').to_string()),
                ),
                None => (header.to_string(), None),
            };
            current = Some(VideoDeviceRecord {
                name,
                bus_hint,
                nodes: Vec::new(),
            });
        } else if let Some(record) = current.as_mut() {
            let node = line.trim();
            if node.starts_with("/dev/video") {
                record.nodes.push(node.to_string());
            }
        }
    }
    if let Some(record) = current.take() {
        records.push(record);
    }
    records
}

pub fn parse_v4l2_list_formats_ext(input: &str) -> Vec<String> {
    let mut capabilities = Vec::new();
    let mut current_format: Option<String> = None;

    for line in input.lines() {
        let trimmed = line.trim();
        if let Some((_, rest)) = trimmed.split_once("]: '") {
            current_format = rest.split_once('\'').map(|(format, _)| format.to_string());
            continue;
        }
        let Some(format) = current_format.as_deref() else {
            continue;
        };
        let Some(size) = trimmed.strip_prefix("Size: Discrete ") else {
            continue;
        };
        let size = size.split_whitespace().next().unwrap_or(size);
        if size.is_empty() {
            continue;
        }
        let capability = format!("{format} {size}");
        if !capabilities.contains(&capability) {
            capabilities.push(capability);
        }
    }

    capabilities
}
