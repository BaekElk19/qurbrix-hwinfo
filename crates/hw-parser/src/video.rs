use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VideoDeviceRecord {
    pub name: String,
    pub bus_hint: Option<String>,
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LshwVideoRecord {
    pub product: Option<String>,
    pub vendor: Option<String>,
    pub logical_name: Option<String>,
    pub bus_info: Option<String>,
    pub driver: Option<String>,
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

pub fn parse_lshw_video(input: &str) -> Vec<LshwVideoRecord> {
    let mut records = Vec::new();
    let mut current: Option<LshwVideoRecord> = None;

    for line in input.lines().chain(std::iter::once("")) {
        let trimmed = line.trim();
        if trimmed.starts_with("*-multimedia") {
            push_lshw_video_record(&mut records, current.take());
            current = Some(LshwVideoRecord::default());
            continue;
        }

        let Some(record) = current.as_mut() else {
            continue;
        };
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "product" => record.product = clean_lshw_video_value(value),
            "vendor" => record.vendor = clean_lshw_video_value(value),
            "logical name" => record.logical_name = clean_lshw_video_value(value),
            "bus info" => record.bus_info = clean_lshw_video_value(value),
            "configuration" => parse_lshw_video_configuration(record, value),
            _ => {}
        }
    }

    push_lshw_video_record(&mut records, current.take());
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

fn push_lshw_video_record(records: &mut Vec<LshwVideoRecord>, record: Option<LshwVideoRecord>) {
    if let Some(record) = record {
        if record
            .logical_name
            .as_deref()
            .is_some_and(|name| name.starts_with("/dev/video"))
        {
            records.push(record);
        }
    }
}

fn parse_lshw_video_configuration(record: &mut LshwVideoRecord, value: &str) {
    for part in value.split_whitespace() {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key == "driver" {
            record.driver = clean_lshw_video_value(value);
        }
    }
}

fn clean_lshw_video_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("n/a") || value == "(none)" {
        None
    } else {
        Some(value.to_string())
    }
}
