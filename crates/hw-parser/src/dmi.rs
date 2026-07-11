use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DmiMemoryRecord {
    pub name: Option<String>,
    pub size: Option<String>,
    pub locator: Option<String>,
    pub manufacturer: Option<String>,
    pub serial: Option<String>,
    pub part_number: Option<String>,
    pub memory_type: Option<String>,
    pub speed: Option<String>,
    pub configured_speed: Option<String>,
    pub total_width: Option<String>,
    pub data_width: Option<String>,
    pub minimum_voltage: Option<String>,
    pub maximum_voltage: Option<String>,
    pub configured_voltage: Option<String>,
    pub error_information_handle: Option<String>,
    pub form_factor: Option<String>,
    pub set: Option<String>,
    pub bank_locator: Option<String>,
    pub type_detail: Option<String>,
    pub asset_tag: Option<String>,
    pub rank: Option<String>,
    pub module_manufacturer_id: Option<String>,
    pub module_product_id: Option<String>,
    pub memory_subsystem_controller_manufacturer_id: Option<String>,
    pub memory_subsystem_controller_product_id: Option<String>,
    pub memory_technology: Option<String>,
    pub memory_operating_mode_capability: Option<String>,
    pub firmware_version: Option<String>,
    pub non_volatile_size: Option<String>,
    pub volatile_size: Option<String>,
    pub cache_size: Option<String>,
    pub logical_size: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DmiBiosBoardRecord {
    pub smbios_version: Option<String>,
    pub bios_vendor: Option<String>,
    pub bios_version: Option<String>,
    pub bios_release_date: Option<String>,
    pub bios_address: Option<String>,
    pub bios_runtime_size: Option<String>,
    pub bios_rom_size: Option<String>,
    pub bios_characteristics: Vec<String>,
    pub bios_revision: Option<String>,
    pub firmware_revision: Option<String>,
    pub bios_language_description_format: Option<String>,
    pub bios_installable_languages: Vec<String>,
    pub bios_currently_installed_language: Option<String>,
    pub board_manufacturer: Option<String>,
    pub board_product_name: Option<String>,
    pub board_version: Option<String>,
    pub board_serial: Option<String>,
    pub board_asset_tag: Option<String>,
    pub chipset_family: Option<String>,
    pub board_features: Vec<String>,
    pub board_type: Option<String>,
    pub board_location_in_chassis: Option<String>,
    pub board_chassis_handle: Option<String>,
    pub chassis_manufacturer: Option<String>,
    pub chassis_type: Option<String>,
    pub chassis_version: Option<String>,
    pub chassis_serial: Option<String>,
    pub chassis_asset_tag: Option<String>,
    pub chassis_lock: Option<String>,
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
            push_memory_record(&mut records, current.take());
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
            "Size" => record.size = clean_memory_size(value),
            "Locator" => record.locator = clean_memory_value(value),
            "Bank Locator" => {
                record.bank_locator = clean_memory_value(value);
                if record.locator.is_none() {
                    record.locator = record.bank_locator.clone();
                }
            }
            "Manufacturer" => record.manufacturer = clean_memory_value(value),
            "Manufacturer ID" => {
                if record.module_manufacturer_id.is_none() {
                    record.module_manufacturer_id = clean_memory_value(value);
                }
                if record.manufacturer.is_none() {
                    record.manufacturer =
                        clean_memory_value(value).map(|value| value.to_uppercase());
                }
            }
            "Serial Number" => record.serial = clean_memory_serial(value),
            "Part Number" => record.part_number = clean_memory_value(value),
            "Type" => record.memory_type = clean_memory_type(value),
            "Speed" => record.speed = clean_memory_value(value),
            "Configured Memory Speed" => {
                record.configured_speed = clean_memory_value(value);
                if record.speed.is_none() {
                    record.speed = record.configured_speed.clone();
                }
            }
            "Total Width" => record.total_width = clean_memory_value(value),
            "Data Width" => record.data_width = clean_memory_value(value),
            "Minimum Voltage" => record.minimum_voltage = clean_memory_value(value),
            "Maximum Voltage" => record.maximum_voltage = clean_memory_value(value),
            "Configured Voltage" => record.configured_voltage = clean_memory_value(value),
            "Error Information Handle" => {
                record.error_information_handle = clean_memory_value(value)
            }
            "Form Factor" => record.form_factor = clean_memory_value(value),
            "Set" => record.set = clean_memory_value(value),
            "Type Detail" => record.type_detail = clean_memory_value(value),
            "Asset Tag" => record.asset_tag = clean_memory_value(value),
            "Rank" => record.rank = clean_memory_value(value),
            "Module Manufacturer ID" => {
                record.module_manufacturer_id = clean_memory_value(value);
                if record.manufacturer.is_none() {
                    record.manufacturer = record
                        .module_manufacturer_id
                        .as_ref()
                        .map(|value| value.to_uppercase());
                }
            }
            "Module Product ID" => record.module_product_id = clean_memory_value(value),
            "Memory Subsystem Controller Manufacturer ID" => {
                record.memory_subsystem_controller_manufacturer_id = clean_memory_value(value)
            }
            "Memory Subsystem Controller Product ID" => {
                record.memory_subsystem_controller_product_id = clean_memory_value(value)
            }
            "Memory Technology" => record.memory_technology = clean_memory_value(value),
            "Memory Operating Mode Capability" => {
                record.memory_operating_mode_capability = clean_memory_value(value)
            }
            "Firmware Version" => record.firmware_version = clean_memory_value(value),
            "Non-Volatile Size" => record.non_volatile_size = clean_memory_value(value),
            "Volatile Size" => record.volatile_size = clean_memory_value(value),
            "Cache Size" => record.cache_size = clean_memory_value(value),
            "Logical Size" => record.logical_size = clean_memory_value(value),
            _ => {}
        }
    }
    push_memory_record(&mut records, current.take());
    records
}

pub fn parse_lshw_memory(input: &str) -> Vec<DmiMemoryRecord> {
    let mut records = Vec::new();
    let mut current: Option<DmiMemoryRecord> = None;
    let mut current_is_bank = false;

    for line in input.lines().chain(std::iter::once("")) {
        let trimmed = line.trim();
        if trimmed.starts_with("*-memory") {
            if current_is_bank {
                push_memory_record(&mut records, current.take());
            }
            current = Some(DmiMemoryRecord::default());
            current_is_bank = false;
            continue;
        }
        if trimmed.starts_with("*-bank") {
            if current_is_bank {
                push_memory_record(&mut records, current.take());
            }
            current = Some(DmiMemoryRecord::default());
            current_is_bank = true;
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
                if record.name.is_none() {
                    record.name = clean_memory_value(value);
                }
                if record.form_factor.is_none() {
                    record.form_factor = lshw_memory_form_factor(value);
                }
                record.memory_type = lshw_memory_type(value);
                if record.speed.is_none() {
                    record.speed = lshw_clock_speed(value);
                }
            }
            "product" => {
                record.part_number = clean_memory_value(value);
                record.name = record.part_number.clone();
            }
            "vendor" => record.manufacturer = clean_memory_value(value),
            "serial" => record.serial = clean_memory_value(value),
            "slot" => record.locator = clean_memory_value(value),
            "size" => record.size = clean_memory_value(value),
            "clock" => record.speed = lshw_clock_speed(value),
            "width" => {
                record.total_width = clean_memory_value(value);
                record.data_width = record.total_width.clone();
            }
            _ => {}
        }
    }

    if current_is_bank || records.is_empty() {
        push_memory_record(&mut records, current.take());
    }
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
        match key.trim() {
            "Manufacturer" => record.manufacturer = clean_dmi_identity_value(value),
            "Product Name" => record.product_name = clean_dmi_identity_value(value),
            "Version" => record.version = clean_dmi_identity_value(value),
            "Serial Number" => record.serial = clean_dmi_identity_value(value),
            "UUID" => record.uuid = clean_dmi_identity_value(value),
            "Wake-up Type" => record.wake_up_type = clean_dmi_value(value),
            "SKU Number" => record.sku_number = clean_dmi_identity_value(value),
            "Family" => record.family = clean_dmi_identity_value(value),
            _ => {}
        }
    }

    record
}

pub fn parse_dmidecode_bios_board(input: &str) -> DmiBiosBoardRecord {
    let mut record = DmiBiosBoardRecord::default();
    let mut section = "";
    let mut collecting_installable_languages = false;
    let mut collecting_bios_characteristics = false;
    let mut collecting_board_features = false;
    for line in input.lines() {
        let trimmed = line.trim();
        if record.smbios_version.is_none() {
            if let Some(version) = parse_smbios_version_line(trimmed) {
                record.smbios_version = Some(version);
            }
        }
        if trimmed == "BIOS Information"
            || trimmed == "BIOS Language Information"
            || trimmed == "Base Board Information"
            || trimmed == "Chassis Information"
            || trimmed == "Physical Memory Array"
        {
            section = trimmed;
            collecting_installable_languages = false;
            collecting_bios_characteristics = false;
            collecting_board_features = false;
            continue;
        }
        if section == "BIOS Information"
            && collecting_bios_characteristics
            && line.starts_with(char::is_whitespace)
            && !trimmed.is_empty()
            && !trimmed.contains(':')
        {
            record.bios_characteristics.push(trimmed.to_string());
            continue;
        }
        if section == "Base Board Information"
            && collecting_board_features
            && line.starts_with(char::is_whitespace)
            && !trimmed.is_empty()
            && !trimmed.contains(':')
        {
            record.board_features.push(trimmed.to_string());
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
        collecting_installable_languages = false;
        collecting_bios_characteristics = false;
        collecting_board_features = false;
        match (section, key) {
            ("BIOS Information", "Vendor") => record.bios_vendor = clean_dmi_identity_value(value),
            ("BIOS Information", "Version") => {
                record.bios_version = clean_dmi_identity_value(value)
            }
            ("BIOS Information", "Release Date") => {
                record.bios_release_date = clean_dmi_value(value)
            }
            ("BIOS Information", "Address") => record.bios_address = clean_dmi_value(value),
            ("BIOS Information", "Runtime Size") => {
                record.bios_runtime_size = clean_dmi_value(value)
            }
            ("BIOS Information", "ROM Size") => record.bios_rom_size = clean_dmi_value(value),
            ("BIOS Information", "Characteristics") => collecting_bios_characteristics = true,
            ("BIOS Information", "BIOS Revision") => record.bios_revision = clean_dmi_value(value),
            ("BIOS Information", "Firmware Revision") => {
                record.firmware_revision = clean_dmi_value(value)
            }
            ("BIOS Language Information", "Language Description Format") => {
                record.bios_language_description_format = clean_dmi_value(value)
            }
            ("BIOS Language Information", "Installable Languages") => {
                collecting_installable_languages = true
            }
            ("BIOS Language Information", "Currently Installed Language") => {
                record.bios_currently_installed_language = clean_dmi_value(value)
            }
            ("Base Board Information", "Manufacturer") => {
                record.board_manufacturer = clean_dmi_identity_value(value)
            }
            ("Base Board Information", "Product Name") => {
                record.board_product_name = clean_dmi_identity_value(value)
            }
            ("Base Board Information", "Version") => {
                record.board_version = clean_dmi_identity_value(value)
            }
            ("Base Board Information", "Serial Number") => {
                record.board_serial = clean_dmi_identity_value(value)
            }
            ("Base Board Information", "Asset Tag") => {
                record.board_asset_tag = clean_dmi_identity_value(value)
            }
            ("Base Board Information", "Features") => collecting_board_features = true,
            ("Base Board Information", "Type") => record.board_type = clean_dmi_value(value),
            ("Base Board Information", "Location In Chassis") => {
                record.board_location_in_chassis = clean_dmi_identity_value(value)
            }
            ("Base Board Information", "Chassis Handle") => {
                record.board_chassis_handle = clean_dmi_value(value)
            }
            ("Chassis Information", "Manufacturer") => {
                record.chassis_manufacturer = clean_dmi_identity_value(value)
            }
            ("Chassis Information", "Type") => record.chassis_type = clean_dmi_value(value),
            ("Chassis Information", "Version") => {
                record.chassis_version = clean_dmi_identity_value(value)
            }
            ("Chassis Information", "Serial Number") => {
                record.chassis_serial = clean_dmi_identity_value(value)
            }
            ("Chassis Information", "Asset Tag") => {
                record.chassis_asset_tag = clean_dmi_identity_value(value)
            }
            ("Chassis Information", "Lock") => record.chassis_lock = clean_dmi_value(value),
            ("Chassis Information", "Boot-up State") => {
                record.chassis_boot_up_state = clean_dmi_value(value)
            }
            ("Chassis Information", "Power Supply State") => {
                record.chassis_power_supply_state = clean_dmi_value(value)
            }
            ("Chassis Information", "Thermal State") => {
                record.chassis_thermal_state = clean_dmi_value(value)
            }
            ("Chassis Information", "Security Status") => {
                record.chassis_security_status = clean_dmi_value(value)
            }
            ("Chassis Information", "OEM Information") => {
                record.chassis_oem_information = clean_dmi_value(value)
            }
            ("Chassis Information", "Height") => record.chassis_height = clean_dmi_value(value),
            ("Chassis Information", "Number Of Power Cords") => {
                record.chassis_power_cords = clean_dmi_value(value)
            }
            ("Chassis Information", "Contained Elements") => {
                record.chassis_contained_elements = clean_dmi_value(value)
            }
            ("Chassis Information", "SKU Number") => {
                record.chassis_sku_number = clean_dmi_identity_value(value)
            }
            ("Physical Memory Array", "Location") => {
                record.memory_array_location = clean_dmi_value(value)
            }
            ("Physical Memory Array", "Use") => record.memory_array_use = clean_dmi_value(value),
            ("Physical Memory Array", "Error Correction Type") => {
                record.memory_array_error_correction_type = clean_dmi_value(value)
            }
            ("Physical Memory Array", "Maximum Capacity") => {
                record.memory_array_maximum_capacity = clean_dmi_value(value)
            }
            ("Physical Memory Array", "Error Information Handle") => {
                record.memory_array_error_information_handle = clean_dmi_value(value)
            }
            ("Physical Memory Array", "Number Of Devices") => {
                record.memory_array_number_of_devices = clean_dmi_value(value)
            }
            _ => {}
        }
    }
    record
}

fn parse_smbios_version_line(value: &str) -> Option<String> {
    let rest = value.strip_prefix("SMBIOS ")?;
    let version = rest.strip_suffix(" present.")?;
    (!version.trim().is_empty()).then(|| version.trim().to_string())
}

fn clean_dmi_value(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn clean_dmi_identity_value(value: &str) -> Option<String> {
    let value = clean_dmi_value(value)?;
    (!matches!(
        value.to_ascii_lowercase().as_str(),
        "none"
            | "n/a"
            | "not specified"
            | "no asset tag"
            | "not settable"
            | "to be filled by o.e.m."
            | "system serial number"
    ))
    .then_some(value)
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

pub fn parse_width_bits(value: Option<&str>) -> Option<u32> {
    value?.split_whitespace().next()?.parse().ok()
}

pub fn parse_voltage_v(value: Option<&str>) -> Option<f32> {
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
    record.name.is_some()
        || record.size.is_some()
        || record.locator.is_some()
        || record.manufacturer.is_some()
        || record.serial.is_some()
        || record.part_number.is_some()
        || record.memory_type.is_some()
        || record.speed.is_some()
        || record.configured_speed.is_some()
        || record.total_width.is_some()
        || record.data_width.is_some()
        || record.minimum_voltage.is_some()
        || record.maximum_voltage.is_some()
        || record.configured_voltage.is_some()
        || record.error_information_handle.is_some()
        || record.form_factor.is_some()
        || record.set.is_some()
        || record.bank_locator.is_some()
        || record.type_detail.is_some()
        || record.asset_tag.is_some()
        || record.rank.is_some()
        || record.module_manufacturer_id.is_some()
        || record.module_product_id.is_some()
        || record.memory_subsystem_controller_manufacturer_id.is_some()
        || record.memory_subsystem_controller_product_id.is_some()
        || record.memory_technology.is_some()
        || record.memory_operating_mode_capability.is_some()
        || record.firmware_version.is_some()
        || record.non_volatile_size.is_some()
        || record.volatile_size.is_some()
        || record.cache_size.is_some()
        || record.logical_size.is_some()
}

fn clean_memory_value(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()
        && !value.eq_ignore_ascii_case("Not Specified")
        && !value.eq_ignore_ascii_case("Unknown"))
    .then(|| value.to_string())
}

fn clean_memory_serial(value: &str) -> Option<String> {
    clean_memory_value(value).filter(|value| value != "0")
}

fn clean_memory_size(value: &str) -> Option<String> {
    let value = clean_memory_value(value)?;
    if value == "No Module Installed" {
        return Some(value);
    }
    let first = value.split_whitespace().next().unwrap_or("");
    if first.len() > 9 && first.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(value)
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
        0x0198 | 0x9801 => "Kingston",
        0x0443 | 0x4304 => "Ramaxel",
        0x04cb | 0xcb04 => "ADATA",
        0x89cd | 0xcd89 => "Longsys",
        0x8968 | 0x6889 => "Kimtigo",
        0x830b | 0x0b83 => "Nanya",
        0x80da | 0xda80 => "Winbond",
        0x04c8 | 0xc804 => "Powerchip",
        0x899b | 0x9b89 => "YMTC",
        0x8a91 | 0x918a => "CXMT",
        0x8a8f | 0x8f8a => "UNIC",
        0x86c8 | 0xc886 => "GigaDevice",
        0x0746 | 0x4607 | 0x0813 | 0x1308 => "Gloway",
        0x081a | 0x1a08 => "UniIC",
        0x8a02 | 0x028a => "KingSpec",
        0x89f7 | 0xf789 => "Netac",
        0x8ab1 | 0xb18a => "Biwin",
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

fn lshw_memory_form_factor(description: &str) -> Option<String> {
    description
        .split_whitespace()
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_alphanumeric()))
        .find(|token| {
            matches!(
                token.to_ascii_uppercase().as_str(),
                "DIMM" | "SODIMM" | "SO-DIMM" | "RDIMM" | "LRDIMM"
            )
        })
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
