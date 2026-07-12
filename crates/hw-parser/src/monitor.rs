use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct XrandrMonitorRecord {
    pub connector: String,
    pub connected: bool,
    pub primary: bool,
    pub resolution: Option<String>,
    pub current_refresh_hz: Option<u16>,
    pub max_resolution: Option<String>,
    pub min_resolution: Option<String>,
    pub support_resolutions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrandrVerboseMonitorRecord {
    pub connector: String,
    pub edid: Vec<u8>,
    pub edid_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HwinfoMonitorRecord {
    pub model: Option<String>,
    pub vendor: Option<String>,
    pub device: Option<String>,
    pub serial: Option<String>,
    pub resolution: Option<String>,
    pub size_mm: Option<(u32, u32)>,
}

pub fn parse_xrandr_query(input: &str) -> Vec<XrandrMonitorRecord> {
    let mut records = Vec::new();
    for line in input.lines() {
        let mut parts = line.split_whitespace();
        let Some(first) = parts.next() else {
            continue;
        };
        let state = parts.next();
        if matches!(state, Some("connected" | "disconnected")) {
            let rest: Vec<&str> = parts.collect();
            let primary = rest.contains(&"primary");
            let resolution = rest
                .iter()
                .find(|part| part.contains('x') && part.contains('+'))
                .map(|value| value.split('+').next().unwrap_or(value).to_string());
            records.push(XrandrMonitorRecord {
                connector: first.to_string(),
                connected: state == Some("connected"),
                primary,
                resolution,
                current_refresh_hz: None,
                max_resolution: None,
                min_resolution: None,
                support_resolutions: Vec::new(),
            });
            continue;
        }

        if let Some(record) = records.last_mut() {
            if record.connected && record.max_resolution.is_none() {
                record.max_resolution = parse_mode_resolution(first);
            }
            if record.connected {
                let refreshes = parse_mode_refreshes(line);
                if record.current_refresh_hz.is_none() {
                    if let Some(rate) = parse_current_refresh_rate(line) {
                        record.current_refresh_hz = Some(rate);
                    }
                }
                if let Some(mode) = parse_mode_resolution(first) {
                    record.min_resolution = Some(mode);
                }
                record.support_resolutions.extend(refreshes);
            }
        }
    }
    records
}

pub fn parse_xrandr_verbose(input: &str) -> Vec<XrandrVerboseMonitorRecord> {
    let mut records = Vec::new();
    let mut connector: Option<String> = None;
    let mut edid_hex = String::new();
    let mut in_edid = false;
    let mut edid_valid = true;

    for line in input.lines() {
        let trimmed = line.trim();
        let mut parts = trimmed.split_whitespace();
        let first = parts.next();
        let state = parts.next();

        if matches!(state, Some("connected" | "disconnected")) {
            if let Some(connector) = connector.take() {
                let edid_bytes = hex_to_bytes(&edid_hex);
                if edid_valid && !edid_bytes.is_empty() {
                    let normalized_hex = edid_hex
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .flat_map(|c| c.to_lowercase())
                        .collect::<String>();
                    records.push(XrandrVerboseMonitorRecord {
                        connector,
                        edid: edid_bytes,
                        edid_hex: normalized_hex,
                    });
                }
            }

            connector = (state == Some("connected")).then(|| first.unwrap_or_default().to_string());
            edid_hex.clear();
            in_edid = false;
            edid_valid = true;
            continue;
        }

        if trimmed == "EDID:" {
            in_edid = true;
            edid_hex.clear();
            edid_valid = true;
            continue;
        }

        if in_edid {
            let is_indented = line
                .chars()
                .next()
                .is_some_and(|value| value.is_whitespace());
            if !is_indented
                || trimmed.is_empty()
                || !trimmed.chars().all(|value| value.is_ascii_hexdigit())
            {
                in_edid = false;
            } else if trimmed.len() % 2 == 0 {
                edid_hex.push_str(trimmed);
            } else {
                edid_valid = false;
                in_edid = false;
            }
        }
    }

    if let Some(connector) = connector {
        let edid_bytes = hex_to_bytes(&edid_hex);
        if edid_valid && !edid_bytes.is_empty() {
            let normalized_hex = edid_hex
                .chars()
                .filter(|c| !c.is_whitespace())
                .flat_map(|c| c.to_lowercase())
                .collect::<String>();
            records.push(XrandrVerboseMonitorRecord {
                connector,
                edid: edid_bytes,
                edid_hex: normalized_hex,
            });
        }
    }

    records
}

pub fn parse_hwinfo_monitor(input: &str) -> Vec<HwinfoMonitorRecord> {
    let mut records = Vec::new();
    let mut section = Vec::new();

    for line in input.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            push_hwinfo_monitor_record(&mut records, parse_hwinfo_monitor_section(&section));
            section.clear();
            continue;
        }
        section.push(line);
    }

    records
}

fn parse_hwinfo_monitor_section(lines: &[&str]) -> Option<HwinfoMonitorRecord> {
    let mut record = HwinfoMonitorRecord::default();
    let mut is_monitor = false;

    for line in lines {
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "Hardware Class" => is_monitor = value == "monitor",
            "Model" => record.model = clean_hwinfo_monitor_value(value),
            "Vendor" | "SubVendor" => record.vendor = clean_hwinfo_monitor_value(value),
            "Device" => record.device = clean_hwinfo_monitor_value(value),
            "Serial ID" => record.serial = clean_hwinfo_monitor_value(value),
            "Resolution" => record.resolution = parse_hwinfo_monitor_resolution(value),
            "Size" | "Display Size" => record.size_mm = parse_hwinfo_monitor_size_mm(value),
            _ => {}
        }
    }

    is_monitor.then_some(record)
}

fn push_hwinfo_monitor_record(
    records: &mut Vec<HwinfoMonitorRecord>,
    record: Option<HwinfoMonitorRecord>,
) {
    if let Some(record) = record {
        if record.model.is_some()
            || record.vendor.is_some()
            || record.device.is_some()
            || record.serial.is_some()
            || record.resolution.is_some()
            || record.size_mm.is_some()
        {
            records.push(record);
        }
    }
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks(2)
        .filter_map(|pair| {
            if pair.len() != 2 {
                return None;
            }
            std::str::from_utf8(pair)
                .ok()
                .and_then(|value| u8::from_str_radix(value, 16).ok())
        })
        .collect()
}

fn clean_hwinfo_monitor_value(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value.split('"').nth(1).unwrap_or(value).trim();
    if value.is_empty() || value.contains("unknown") {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_hwinfo_monitor_resolution(value: &str) -> Option<String> {
    let value = clean_hwinfo_monitor_value(value)?;
    value.split_whitespace().find_map(|part| {
        let part = part.split('@').next().unwrap_or(part);
        parse_mode_resolution(part)
    })
}

fn parse_hwinfo_monitor_size_mm(value: &str) -> Option<(u32, u32)> {
    let value = clean_hwinfo_monitor_value(value)?;
    let mut numbers = Vec::new();
    let mut current = String::new();
    for ch in value.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(number) = current.parse() {
                numbers.push(number);
            }
            current.clear();
        }
    }
    (numbers.len() >= 2).then_some((numbers[0], numbers[1]))
}

fn parse_mode_resolution(value: &str) -> Option<String> {
    let (width, height) = value.split_once('x')?;
    (!width.is_empty()
        && !height.is_empty()
        && width.chars().all(|c| c.is_ascii_digit())
        && height.chars().all(|c| c.is_ascii_digit()))
    .then(|| value.to_string())
}

fn parse_current_refresh_rate(line: &str) -> Option<u16> {
    let mut parts = line.split_whitespace();
    let _resolution = parts.next().and_then(parse_mode_resolution)?;
    for part in parts {
        if !part.contains('*') {
            continue;
        }
        let rate = part.trim_end_matches(['*', '+']);
        if rate.is_empty() || !rate.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
            continue;
        }
        if let Ok(value) = rate.parse::<f32>() {
            return Some(value.round() as u16);
        }
    }
    None
}

fn parse_mode_refreshes(line: &str) -> Vec<String> {
    let mut parts = line.split_whitespace();
    let Some(resolution) = parts.next().and_then(parse_mode_resolution) else {
        return Vec::new();
    };
    parts
        .filter_map(|part| {
            let rate = part.trim_end_matches(['*', '+']);
            if rate.is_empty() || !rate.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
                return None;
            }
            Some(format!("{resolution}@{}Hz", normalize_refresh_rate(rate)))
        })
        .collect()
}

fn normalize_refresh_rate(rate: &str) -> String {
    let trimmed = rate.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}
