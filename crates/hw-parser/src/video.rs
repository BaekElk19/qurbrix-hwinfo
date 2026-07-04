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
