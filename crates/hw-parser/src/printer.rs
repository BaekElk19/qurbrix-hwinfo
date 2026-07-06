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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrinterDetailRecord {
    pub queue: String,
    pub make_model: Option<String>,
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

pub fn parse_lpstat_l_p(input: &str) -> Vec<PrinterDetailRecord> {
    let mut records = Vec::new();
    let mut current: Option<PrinterDetailRecord> = None;

    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("printer ") {
            if let Some(record) = current.take() {
                records.push(record);
            }
            current = rest
                .split_whitespace()
                .next()
                .map(|queue| PrinterDetailRecord {
                    queue: queue.to_string(),
                    make_model: None,
                });
            continue;
        }

        let Some(record) = current.as_mut() else {
            continue;
        };
        let line = line.trim();
        if let Some(value) = line
            .strip_prefix("Description:")
            .or_else(|| line.strip_prefix("Make and Model:"))
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            record.make_model = Some(value.to_string());
        }
    }

    if let Some(record) = current {
        records.push(record);
    }
    records
}

pub fn parse_lpstat_d(input: &str) -> Option<String> {
    input.lines().find_map(|line| {
        let value = line.strip_prefix("system default destination:")?.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}
