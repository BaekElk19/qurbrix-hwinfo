use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PowerRecord {
    pub native_path: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub state: Option<String>,
    pub technology: Option<String>,
    pub capacity_percent: Option<f32>,
    pub energy_full_wh: Option<f32>,
    pub energy_design_wh: Option<f32>,
    pub energy_now_wh: Option<f32>,
    pub voltage_v: Option<f32>,
    pub present: Option<bool>,
}

pub fn parse_upower_dump(input: &str) -> Vec<PowerRecord> {
    let mut records = Vec::new();
    let mut current: Option<PowerRecord> = None;
    for line in input.lines() {
        if line.starts_with("Device: ") {
            if let Some(record) = current.take() {
                records.push(record);
            }
            current = Some(PowerRecord::default());
            continue;
        }
        let Some(record) = current.as_mut() else {
            continue;
        };
        let line = line.trim();
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            match key.trim() {
                "native-path" => record.native_path = Some(value.to_string()),
                "vendor" => record.vendor = Some(value.to_string()),
                "model" => record.model = Some(value.to_string()),
                "serial" => record.serial = Some(value.to_string()),
                "present" => record.present = Some(value == "yes"),
                "state" => record.state = Some(value.to_string()),
                "technology" => record.technology = Some(value.to_string()),
                "capacity" => record.capacity_percent = parse_number(value),
                "energy-full" => record.energy_full_wh = parse_number(value),
                "energy-full-design" => record.energy_design_wh = parse_number(value),
                "energy" => record.energy_now_wh = parse_number(value),
                "voltage" => record.voltage_v = parse_number(value),
                _ => {}
            }
        }
    }
    if let Some(record) = current.take() {
        records.push(record);
    }
    records
}

fn parse_number(value: &str) -> Option<f32> {
    value
        .split_whitespace()
        .next()?
        .trim_end_matches('%')
        .parse()
        .ok()
}
