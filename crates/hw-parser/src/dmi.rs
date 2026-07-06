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
    pub board_version: Option<String>,
    pub board_serial: Option<String>,
    pub board_asset_tag: Option<String>,
    pub board_location_in_chassis: Option<String>,
    pub board_chassis_handle: Option<String>,
    pub chassis_manufacturer: Option<String>,
    pub chassis_type: Option<String>,
    pub chassis_version: Option<String>,
    pub chassis_serial: Option<String>,
    pub chassis_asset_tag: Option<String>,
    pub chassis_boot_up_state: Option<String>,
    pub chassis_power_supply_state: Option<String>,
    pub chassis_thermal_state: Option<String>,
    pub chassis_security_status: Option<String>,
    pub chassis_oem_information: Option<String>,
    pub chassis_height: Option<String>,
    pub chassis_power_cords: Option<String>,
    pub chassis_contained_elements: Option<String>,
    pub chassis_sku_number: Option<String>,
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

pub fn parse_lshw_memory(input: &str) -> Vec<DmiMemoryRecord> {
    let mut records = Vec::new();
    let mut current: Option<DmiMemoryRecord> = None;

    for line in input.lines().chain(std::iter::once("")) {
        let trimmed = line.trim();
        if trimmed.starts_with("*-bank") {
            push_memory_record(&mut records, current.take());
            current = Some(DmiMemoryRecord::default());
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
            "description" => {
                record.memory_type = lshw_memory_type(value);
                if record.speed.is_none() {
                    record.speed = lshw_clock_speed(value);
                }
            }
            "product" => record.part_number = clean_memory_value(value),
            "vendor" => record.manufacturer = clean_memory_value(value),
            "serial" => record.serial = clean_memory_value(value),
            "slot" => record.locator = clean_memory_value(value),
            "size" => record.size = clean_memory_value(value),
            "clock" => record.speed = lshw_clock_speed(value),
            _ => {}
        }
    }

    push_memory_record(&mut records, current.take());
    records
}

pub fn parse_dmidecode_bios_board(input: &str) -> DmiBiosBoardRecord {
    let mut record = DmiBiosBoardRecord::default();
    let mut section = "";
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed == "BIOS Information"
            || trimmed == "Base Board Information"
            || trimmed == "Chassis Information"
        {
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
            ("Base Board Information", "Version") => record.board_version = Some(value),
            ("Base Board Information", "Serial Number") => record.board_serial = Some(value),
            ("Base Board Information", "Asset Tag") => record.board_asset_tag = Some(value),
            ("Base Board Information", "Location In Chassis") => {
                record.board_location_in_chassis = Some(value)
            }
            ("Base Board Information", "Chassis Handle") => {
                record.board_chassis_handle = Some(value)
            }
            ("Chassis Information", "Manufacturer") => record.chassis_manufacturer = Some(value),
            ("Chassis Information", "Type") => record.chassis_type = Some(value),
            ("Chassis Information", "Version") => record.chassis_version = Some(value),
            ("Chassis Information", "Serial Number") => record.chassis_serial = Some(value),
            ("Chassis Information", "Asset Tag") => record.chassis_asset_tag = Some(value),
            ("Chassis Information", "Boot-up State") => record.chassis_boot_up_state = Some(value),
            ("Chassis Information", "Power Supply State") => {
                record.chassis_power_supply_state = Some(value)
            }
            ("Chassis Information", "Thermal State") => record.chassis_thermal_state = Some(value),
            ("Chassis Information", "Security Status") => {
                record.chassis_security_status = Some(value)
            }
            ("Chassis Information", "OEM Information") => {
                record.chassis_oem_information = Some(value)
            }
            ("Chassis Information", "Height") => record.chassis_height = Some(value),
            ("Chassis Information", "Number Of Power Cords") => {
                record.chassis_power_cords = Some(value)
            }
            ("Chassis Information", "Contained Elements") => {
                record.chassis_contained_elements = Some(value)
            }
            ("Chassis Information", "SKU Number") => record.chassis_sku_number = Some(value),
            _ => {}
        }
    }
    record
}

pub fn parse_size_to_bytes(value: Option<&str>) -> Option<u64> {
    let value = value?;
    let mut parts = value.split_whitespace();
    let first = parts.next()?;
    let (number, unit) = match first.parse::<u64>() {
        Ok(number) => (number, parts.next().unwrap_or("").to_string()),
        Err(_) => {
            let split = first.find(|c: char| !c.is_ascii_digit())?;
            if split == 0 {
                return None;
            }
            (
                first[..split].parse::<u64>().ok()?,
                first[split..].to_string(),
            )
        }
    };
    let unit = unit.to_ascii_lowercase();
    match unit.as_str() {
        "kb" | "kib" => Some(number * 1024),
        "mb" | "mib" => Some(number * 1024 * 1024),
        "gb" | "gib" => Some(number * 1024 * 1024 * 1024),
        "tb" | "tib" => Some(number * 1024 * 1024 * 1024 * 1024),
        _ => Some(number),
    }
}

pub fn parse_proc_meminfo_total_bytes(input: &str) -> Option<u64> {
    let line = input
        .lines()
        .find(|line| line.trim_start().starts_with("MemTotal:"))?;
    let mut parts = line.split_whitespace();
    (parts.next()? == "MemTotal:").then_some(())?;
    let kib = parts.next()?.parse::<u64>().ok()?;
    (parts.next()? == "kB").then_some(kib * 1024)
}

pub fn parse_speed_mtps(value: Option<&str>) -> Option<u32> {
    value?.split_whitespace().next()?.parse().ok()
}

fn push_memory_record(records: &mut Vec<DmiMemoryRecord>, record: Option<DmiMemoryRecord>) {
    let Some(record) = record else {
        return;
    };
    let has_data = record.size.is_some()
        || record.locator.is_some()
        || record.manufacturer.is_some()
        || record.serial.is_some()
        || record.part_number.is_some()
        || record.memory_type.is_some()
        || record.speed.is_some();
    if has_data && record.size.as_deref() != Some("No Module Installed") {
        records.push(record);
    }
}

fn clean_memory_value(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && !value.eq_ignore_ascii_case("Not Specified")).then(|| value.to_string())
}

fn lshw_memory_type(description: &str) -> Option<String> {
    description
        .split_whitespace()
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_alphanumeric()))
        .find(|token| token.to_ascii_uppercase().starts_with("DDR"))
        .map(str::to_string)
}

fn lshw_clock_speed(value: &str) -> Option<String> {
    let start = value.find(|c: char| c.is_ascii_digit())?;
    let digits: String = value[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    (!digits.is_empty()).then(|| format!("{digits} MT/s"))
}
