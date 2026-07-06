use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct XrandrMonitorRecord {
    pub connector: String,
    pub connected: bool,
    pub primary: bool,
    pub resolution: Option<String>,
    pub max_resolution: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrandrVerboseMonitorRecord {
    pub connector: String,
    pub edid: Vec<u8>,
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
                max_resolution: None,
            });
            continue;
        }

        if let Some(record) = records.last_mut() {
            if record.connected && record.max_resolution.is_none() {
                record.max_resolution = parse_mode_resolution(first);
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
                let edid = hex_to_bytes(&edid_hex);
                if edid_valid && !edid.is_empty() {
                    records.push(XrandrVerboseMonitorRecord { connector, edid });
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
        let edid = hex_to_bytes(&edid_hex);
        if edid_valid && !edid.is_empty() {
            records.push(XrandrVerboseMonitorRecord { connector, edid });
        }
    }

    records
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

fn parse_mode_resolution(value: &str) -> Option<String> {
    let (width, height) = value.split_once('x')?;
    (!width.is_empty()
        && !height.is_empty()
        && width.chars().all(|c| c.is_ascii_digit())
        && height.chars().all(|c| c.is_ascii_digit()))
    .then(|| value.to_string())
}
