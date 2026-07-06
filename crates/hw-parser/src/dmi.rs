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
    pub bios_language_description_format: Option<String>,
    pub bios_installable_languages: Vec<String>,
    pub bios_currently_installed_language: Option<String>,
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
    pub memory_array_location: Option<String>,
    pub memory_array_use: Option<String>,
    pub memory_array_error_correction_type: Option<String>,
    pub memory_array_maximum_capacity: Option<String>,
    pub memory_array_error_information_handle: Option<String>,
    pub memory_array_number_of_devices: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DmiSystemRecord {
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
    pub version: Option<String>,
    pub serial: Option<String>,
    pub uuid: Option<String>,
    pub wake_up_type: Option<String>,
    pub sku_number: Option<String>,
    pub family: Option<String>,
}

impl DmiSystemRecord {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
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
            "Locator" => record.locator = clean_memory_value(value),
            "Bank Locator" => {
                if record.locator.is_none() {
                    record.locator = clean_memory_value(value);
                }
            }
            "Manufacturer" => record.manufacturer = clean_memory_value(value),
            "Manufacturer ID" | "Module Manufacturer ID" => {
                if record.manufacturer.is_none() {
                    record.manufacturer =
                        clean_memory_value(value).map(|value| value.to_uppercase());
                }
            }
            "Serial Number" => record.serial = Some(value.to_string()),
            "Part Number" => record.part_number = Some(value.to_string()),
            "Type" => record.memory_type = clean_memory_type(value),
            "Speed" => record.speed = clean_memory_value(value),
            "Configured Memory Speed" => {
                if record.speed.is_none() {
                    record.speed = clean_memory_value(value);
                }
            }
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

pub fn parse_spd_decode_dimms(input: &str) -> Vec<DmiMemoryRecord> {
    let mut records = Vec::new();
    let mut current: Option<DmiMemoryRecord> = None;

    for line in input.lines() {
        let trimmed = line.trim();
        if let Some(path) = trimmed.strip_prefix("Decoding EEPROM:") {
            push_memory_record(&mut records, current.take());
            current = Some(DmiMemoryRecord {
                locator: clean_memory_value(path),
                ..Default::default()
            });
            continue;
        }

        let Some(record) = current.as_mut() else {
            continue;
        };

        if let Some(value) = decode_dimms_value(trimmed, "Guessing DIMM is in") {
            record.locator = clean_memory_value(value);
        } else if let Some(value) = decode_dimms_value(trimmed, "Fundamental Memory type") {
            record.memory_type = clean_memory_value(value);
        } else if let Some(value) = decode_dimms_value(trimmed, "Maximum module speed") {
            record.speed = clean_memory_value(value);
        } else if let Some(value) = decode_dimms_value(trimmed, "Size") {
            record.size = clean_memory_value(value);
        } else if let Some(value) = decode_dimms_value(trimmed, "Module Manufacturer") {
            record.manufacturer = clean_memory_value(value);
        } else if let Some(value) = decode_dimms_value(trimmed, "Assembly Serial Number") {
            record.serial = clean_memory_value(value);
        } else if let Some(value) = decode_dimms_value(trimmed, "Part Number") {
            record.part_number = clean_memory_value(value);
        }
    }

    push_memory_record(&mut records, current.take());
    records
}

pub fn parse_spd_eeprom(bytes: &[u8]) -> Option<DmiMemoryRecord> {
    match bytes.get(2).copied()? {
        0x0c => parse_ddr4_spd_eeprom(bytes),
        0x12 => parse_ddr5_spd_eeprom(bytes),
        _ => None,
    }
}

fn parse_ddr4_spd_eeprom(bytes: &[u8]) -> Option<DmiMemoryRecord> {
    let size = ddr4_spd_size(bytes).map(|size_mib| format!("{size_mib} MB"));
    let speed = ddr4_spd_speed(bytes).map(|speed_mtps| format!("{speed_mtps} MT/s"));
    let manufacturer = bytes.get(320..322).and_then(spd_manufacturer_name);
    let serial = bytes.get(323..327).and_then(spd_hex_string);
    let part_number = spd_ascii_string(bytes.get(329..349).unwrap_or_default());
    let has_spd_data = size.is_some()
        || speed.is_some()
        || manufacturer.is_some()
        || serial.is_some()
        || part_number.is_some();

    let record = DmiMemoryRecord {
        size,
        manufacturer,
        serial,
        part_number,
        memory_type: Some("DDR4 SDRAM".to_string()),
        speed,
        ..Default::default()
    };
    has_spd_data.then_some(record)
}

fn parse_ddr5_spd_eeprom(bytes: &[u8]) -> Option<DmiMemoryRecord> {
    let manufacturer = bytes.get(512..514).and_then(spd_manufacturer_name);
    let serial = bytes.get(517..521).and_then(spd_hex_string);
    let part_number = spd_ascii_string(bytes.get(521..551).unwrap_or_default());
    let has_spd_data = manufacturer.is_some() || serial.is_some() || part_number.is_some();

    let record = DmiMemoryRecord {
        manufacturer,
        serial,
        part_number,
        memory_type: Some("DDR5 SDRAM".to_string()),
        ..Default::default()
    };
    has_spd_data.then_some(record)
}

pub fn parse_dmidecode_system(input: &str) -> DmiSystemRecord {
    let mut record = DmiSystemRecord::default();
    let mut in_system = false;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed == "System Information" {
            in_system = true;
            continue;
        }
        if in_system
            && !line.starts_with(char::is_whitespace)
            && !trimmed.is_empty()
            && !trimmed.contains(':')
        {
            in_system = false;
        }
        if !in_system {
            continue;
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim().to_string();
        match key.trim() {
            "Manufacturer" => record.manufacturer = Some(value),
            "Product Name" => record.product_name = Some(value),
            "Version" => record.version = Some(value),
            "Serial Number" => record.serial = Some(value),
            "UUID" => record.uuid = Some(value),
            "Wake-up Type" => record.wake_up_type = Some(value),
            "SKU Number" => record.sku_number = Some(value),
            "Family" => record.family = Some(value),
            _ => {}
        }
    }

    record
}

pub fn parse_dmidecode_bios_board(input: &str) -> DmiBiosBoardRecord {
    let mut record = DmiBiosBoardRecord::default();
    let mut section = "";
    let mut collecting_installable_languages = false;
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed == "BIOS Information"
            || trimmed == "BIOS Language Information"
            || trimmed == "Base Board Information"
            || trimmed == "Chassis Information"
            || trimmed == "Physical Memory Array"
        {
            section = trimmed;
            collecting_installable_languages = false;
            continue;
        }
        if section == "BIOS Language Information"
            && collecting_installable_languages
            && line.starts_with(char::is_whitespace)
            && !trimmed.is_empty()
            && !trimmed.contains(':')
        {
            record.bios_installable_languages.push(trimmed.to_string());
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim().to_string();
        collecting_installable_languages = false;
        match (section, key) {
            ("BIOS Information", "Vendor") => record.bios_vendor = Some(value),
            ("BIOS Information", "Version") => record.bios_version = Some(value),
            ("BIOS Information", "Release Date") => record.bios_release_date = Some(value),
            ("BIOS Language Information", "Language Description Format") => {
                record.bios_language_description_format = Some(value)
            }
            ("BIOS Language Information", "Installable Languages") => {
                collecting_installable_languages = true
            }
            ("BIOS Language Information", "Currently Installed Language") => {
                record.bios_currently_installed_language = Some(value)
            }
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
            ("Physical Memory Array", "Location") => record.memory_array_location = Some(value),
            ("Physical Memory Array", "Use") => record.memory_array_use = Some(value),
            ("Physical Memory Array", "Error Correction Type") => {
                record.memory_array_error_correction_type = Some(value)
            }
            ("Physical Memory Array", "Maximum Capacity") => {
                record.memory_array_maximum_capacity = Some(value)
            }
            ("Physical Memory Array", "Error Information Handle") => {
                record.memory_array_error_information_handle = Some(value)
            }
            ("Physical Memory Array", "Number Of Devices") => {
                record.memory_array_number_of_devices = Some(value)
            }
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
    if memory_record_has_data(&record) && record.size.as_deref() != Some("No Module Installed") {
        records.push(record);
    }
}

fn memory_record_has_data(record: &DmiMemoryRecord) -> bool {
    record.size.is_some()
        || record.locator.is_some()
        || record.manufacturer.is_some()
        || record.serial.is_some()
        || record.part_number.is_some()
        || record.memory_type.is_some()
        || record.speed.is_some()
}

fn clean_memory_value(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()
        && !value.eq_ignore_ascii_case("Not Specified")
        && !value.eq_ignore_ascii_case("Unknown"))
    .then(|| value.to_string())
}

fn clean_memory_type(value: &str) -> Option<String> {
    clean_memory_value(value).filter(|value| value != "<OUT OF SPEC>")
}

fn ddr4_spd_size(bytes: &[u8]) -> Option<u64> {
    let density_code = bytes.get(4)? & 0x0f;
    let organization = *bytes.get(12)?;
    let bus = *bytes.get(13)?;
    let sdram_width_code = organization & 0x07;
    let rank_count = ((organization >> 3) & 0x07) + 1;
    let bus_width_code = bus & 0x07;

    if density_code > 7 || sdram_width_code > 3 || bus_width_code > 3 {
        return None;
    }

    let sdram_mbit = 256u64 << density_code;
    let sdram_width = 4u64 << sdram_width_code;
    let bus_width = 8u64 << bus_width_code;
    if bus_width < sdram_width {
        return None;
    }
    Some((sdram_mbit / 8) * (bus_width / sdram_width) * u64::from(rank_count))
}

fn ddr4_spd_speed(bytes: &[u8]) -> Option<u32> {
    let tck_mtb = u64::from(*bytes.get(18)?);
    (tck_mtb > 0).then_some(())?;
    let tck_ps = tck_mtb * 125;
    u32::try_from((2_000_000 + tck_ps / 2) / tck_ps).ok()
}

fn spd_hex_string(bytes: &[u8]) -> Option<String> {
    let value = bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>();
    clean_memory_value(&value).filter(|value| !value.chars().all(|ch| ch == '0' || ch == 'F'))
}

fn spd_manufacturer_name(bytes: &[u8]) -> Option<String> {
    let id = u16::from_be_bytes(bytes.try_into().ok()?);
    if id == 0 || id == 0xffff {
        return None;
    }
    let name = match id {
        0x80ce | 0xce00 => "Samsung",
        0x80ad | 0xad00 => "SK Hynix",
        0x802c | 0x2c00 => "Micron",
        0x859b | 0x9b00 => "Crucial",
        _ => return Some(format!("JEP106 0x{id:04X}")),
    };
    Some(name.to_string())
}

fn spd_ascii_string(bytes: &[u8]) -> Option<String> {
    let value: String = bytes
        .iter()
        .copied()
        .take_while(|byte| *byte != 0x00 && *byte != 0xff)
        .map(char::from)
        .collect();
    clean_memory_value(&value)
}

fn decode_dimms_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let value = line.strip_prefix(key)?;
    value
        .starts_with(char::is_whitespace)
        .then_some(value.trim())
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
