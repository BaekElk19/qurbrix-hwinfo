use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsoundCardRecord {
    pub index: u32,
    pub id: Option<String>,
    pub name: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LshwMultimediaRecord {
    pub product: Option<String>,
    pub vendor: Option<String>,
    pub bus_info: Option<String>,
    pub driver: Option<String>,
}

pub fn parse_proc_asound_cards(input: &str) -> Vec<AsoundCardRecord> {
    let re = Regex::new(r"^\s*(\d+)\s+\[(.*?)\s*\]:\s*(.*?)\s+-\s+(.*)$").unwrap();
    let mut cards = Vec::new();
    let mut lines = input.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(caps) = re.captures(line) {
            let detail = lines
                .peek()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            cards.push(AsoundCardRecord {
                index: caps[1].parse().unwrap_or(0),
                id: Some(caps[2].trim().to_string()),
                name: Some(caps[4].trim().to_string()),
                detail,
            });
        }
    }
    cards
}

pub fn parse_lshw_multimedia(input: &str) -> Vec<LshwMultimediaRecord> {
    let mut records = Vec::new();
    let mut current: Option<LshwMultimediaRecord> = None;

    for line in input.lines().chain(std::iter::once("")) {
        let trimmed = line.trim();
        if trimmed.starts_with("*-multimedia") {
            push_lshw_multimedia_record(&mut records, current.take());
            current = Some(LshwMultimediaRecord::default());
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
            "product" => record.product = clean_lshw_multimedia_value(value),
            "vendor" => record.vendor = clean_lshw_multimedia_value(value),
            "bus info" => record.bus_info = clean_lshw_multimedia_value(value),
            "configuration" => parse_lshw_multimedia_configuration(record, value),
            _ => {}
        }
    }

    push_lshw_multimedia_record(&mut records, current.take());
    records
}

fn push_lshw_multimedia_record(
    records: &mut Vec<LshwMultimediaRecord>,
    record: Option<LshwMultimediaRecord>,
) {
    if let Some(record) = record {
        if record.product.is_some() || record.vendor.is_some() || record.bus_info.is_some() {
            records.push(record);
        }
    }
}

fn parse_lshw_multimedia_configuration(record: &mut LshwMultimediaRecord, value: &str) {
    for part in value.split_whitespace() {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key == "driver" {
            record.driver = clean_lshw_multimedia_value(value);
        }
    }
}

fn clean_lshw_multimedia_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("n/a") || value == "(none)" {
        None
    } else {
        Some(value.to_string())
    }
}
