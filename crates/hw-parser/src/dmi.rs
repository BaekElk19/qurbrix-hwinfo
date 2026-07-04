use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DmiMemoryRecord {
    pub size: Option<String>,
    pub locator: Option<String>,
    pub manufacturer: Option<String>,
    pub serial: Option<String>,
    pub part_number: Option<String>,
    pub memory_type: Option<String>,
    pub speed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DmiBiosBoardRecord {
    pub bios_vendor: Option<String>,
    pub bios_version: Option<String>,
    pub bios_release_date: Option<String>,
    pub board_manufacturer: Option<String>,
    pub board_product_name: Option<String>,
    pub board_serial: Option<String>,
}

pub fn parse_dmidecode_memory(input: &str) -> Vec<DmiMemoryRecord> {
    let mut records = Vec::new();
    let mut current: Option<DmiMemoryRecord> = None;
    for line in input.lines().chain(std::iter::once("")) {
        if line.trim() == "Memory Device" {
            if let Some(record) = current.take() {
                if record.size.as_deref() != Some("No Module Installed") {
                    records.push(record);
                }
            }
            current = Some(DmiMemoryRecord::default());
            continue;
        }
        let Some(record) = current.as_mut() else {
            continue;
        };
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key {
            "Size" => record.size = Some(value.to_string()),
            "Locator" => record.locator = Some(value.to_string()),
            "Manufacturer" => record.manufacturer = Some(value.to_string()),
            "Serial Number" => record.serial = Some(value.to_string()),
            "Part Number" => record.part_number = Some(value.to_string()),
            "Type" => record.memory_type = Some(value.to_string()),
            "Speed" => record.speed = Some(value.to_string()),
            _ => {}
        }
    }
    if let Some(record) = current.take() {
        if record.size.as_deref() != Some("No Module Installed") {
            records.push(record);
        }
    }
    records
}

pub fn parse_dmidecode_bios_board(input: &str) -> DmiBiosBoardRecord {
    let mut record = DmiBiosBoardRecord::default();
    let mut section = "";
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed == "BIOS Information" || trimmed == "Base Board Information" {
            section = trimmed;
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim().to_string();
        match (section, key) {
            ("BIOS Information", "Vendor") => record.bios_vendor = Some(value),
            ("BIOS Information", "Version") => record.bios_version = Some(value),
            ("BIOS Information", "Release Date") => record.bios_release_date = Some(value),
            ("Base Board Information", "Manufacturer") => record.board_manufacturer = Some(value),
            ("Base Board Information", "Product Name") => record.board_product_name = Some(value),
            ("Base Board Information", "Serial Number") => record.board_serial = Some(value),
            _ => {}
        }
    }
    record
}

pub fn parse_size_to_bytes(value: Option<&str>) -> Option<u64> {
    let value = value?;
    let mut parts = value.split_whitespace();
    let number = parts.next()?.parse::<u64>().ok()?;
    let unit = parts.next().unwrap_or("").to_ascii_lowercase();
    match unit.as_str() {
        "kb" | "kib" => Some(number * 1024),
        "mb" | "mib" => Some(number * 1024 * 1024),
        "gb" | "gib" => Some(number * 1024 * 1024 * 1024),
        "tb" | "tib" => Some(number * 1024 * 1024 * 1024 * 1024),
        _ => Some(number),
    }
}

pub fn parse_speed_mtps(value: Option<&str>) -> Option<u32> {
    value?.split_whitespace().next()?.parse().ok()
}
