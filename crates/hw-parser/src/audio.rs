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
    pub description: Option<String>,
    pub version: Option<String>,
    pub bus_info: Option<String>,
    pub driver: Option<String>,
    pub irq: Option<String>,
    pub capabilities: Vec<String>,
    pub memory_address: Option<String>,
    pub latency: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HwinfoSoundRecord {
    pub model: Option<String>,
    pub vendor: Option<String>,
    pub device: Option<String>,
    pub driver: Option<String>,
    pub driver_modules: Vec<String>,
    pub pci_address: Option<String>,
    pub card_index: Option<u32>,
    pub revision: Option<String>,
    pub irq: Option<String>,
    pub memory_address: Option<String>,
    pub driver_status: Option<String>,
    pub sub_device: Option<String>,
    pub sub_vendor: Option<String>,
    pub modalias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PactlCardProfileRecord {
    pub card_index: Option<u32>,
    pub profiles: Vec<String>,
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
            "description" => record.description = clean_lshw_multimedia_value(value),
            "version" => record.version = clean_lshw_multimedia_value(value),
            "bus info" => record.bus_info = clean_lshw_multimedia_value(value),
            "capabilities" => {
                record.capabilities = value
                    .split_whitespace()
                    .filter_map(clean_lshw_multimedia_value)
                    .collect()
            }
            "configuration" => parse_lshw_multimedia_configuration(record, value),
            "resources" => parse_lshw_multimedia_resources(record, value),
            _ => {}
        }
    }

    push_lshw_multimedia_record(&mut records, current.take());
    records
}

pub fn parse_hwinfo_sound(input: &str) -> Vec<HwinfoSoundRecord> {
    let mut records = Vec::new();
    let mut section = Vec::new();

    for line in input.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            push_hwinfo_sound_record(&mut records, parse_hwinfo_sound_section(&section));
            section.clear();
            continue;
        }
        section.push(line);
    }

    records
}

pub fn parse_pactl_card_profiles(input: &str) -> Vec<PactlCardProfileRecord> {
    let mut records = Vec::new();
    let mut current: Option<PactlCardProfileRecord> = None;
    let mut in_profiles = false;

    for line in input.lines().chain(std::iter::once("Card #")) {
        let trimmed = line.trim();
        if trimmed.starts_with("Card #") {
            push_pactl_card_profile_record(&mut records, current.take());
            current = Some(PactlCardProfileRecord::default());
            in_profiles = false;
            continue;
        }

        let Some(record) = current.as_mut() else {
            continue;
        };
        if let Some(value) = trimmed.strip_prefix("alsa.card = ") {
            record.card_index = value.trim_matches('"').parse().ok();
            continue;
        }
        if trimmed == "Profiles:" {
            in_profiles = true;
            continue;
        }
        if in_profiles {
            if trimmed.ends_with(':') || trimmed.starts_with("Active Profile:") {
                in_profiles = false;
                continue;
            }
            if let Some((profile, _)) = trimmed.split_once(": ") {
                if !profile.is_empty() && !record.profiles.iter().any(|item| item == profile) {
                    record.profiles.push(profile.to_string());
                }
            }
        }
    }

    records
}

fn parse_hwinfo_sound_section(lines: &[&str]) -> Option<HwinfoSoundRecord> {
    let mut record = HwinfoSoundRecord::default();
    let mut is_sound = false;
    let raw_section = lines.join("\n");

    for line in lines {
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "Hardware Class" => is_sound = value == "sound",
            "Model" => record.model = clean_hwinfo_value(value),
            "Vendor" => record.vendor = clean_hwinfo_value(value),
            "Device" => record.device = clean_hwinfo_value(value),
            "Driver" => record.driver = clean_hwinfo_value(value),
            "Driver Modules" => record.driver_modules = clean_hwinfo_modules(value),
            "SysFS BusID" => record.pci_address = clean_hwinfo_pci_address(value),
            "SysFS ID" => record.card_index = parse_hwinfo_sound_card_index(value),
            "Revision" => record.revision = clean_hwinfo_value(value),
            "IRQ" => record.irq = clean_hwinfo_value(value),
            "Memory Range" => record.memory_address = clean_hwinfo_value(value),
            "Driver Status" => record.driver_status = clean_hwinfo_value(value),
            "SubDevice" => record.sub_device = clean_hwinfo_value(value),
            "SubVendor" => record.sub_vendor = clean_hwinfo_value(value),
            "Module Alias" => record.modalias = clean_hwinfo_value(value),
            _ => {}
        }
    }

    let usb_audio_fallback =
        raw_section.contains("USB Audio") && raw_section.contains("snd-usb-audio");
    if is_sound || usb_audio_fallback {
        Some(record)
    } else {
        None
    }
}

fn push_hwinfo_sound_record(
    records: &mut Vec<HwinfoSoundRecord>,
    record: Option<HwinfoSoundRecord>,
) {
    if let Some(record) = record {
        if record.model.is_some()
            || record.vendor.is_some()
            || record.device.is_some()
            || record.driver.is_some()
            || !record.driver_modules.is_empty()
            || record.pci_address.is_some()
            || record.card_index.is_some()
        {
            records.push(record);
        }
    }
}

fn push_pactl_card_profile_record(
    records: &mut Vec<PactlCardProfileRecord>,
    record: Option<PactlCardProfileRecord>,
) {
    if let Some(record) = record {
        if record.card_index.is_some() || !record.profiles.is_empty() {
            records.push(record);
        }
    }
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
        match key {
            "driver" => record.driver = clean_lshw_multimedia_value(value),
            "irq" => record.irq = clean_lshw_multimedia_value(value),
            "latency" => record.latency = clean_lshw_multimedia_value(value),
            _ => {}
        }
    }
}

fn parse_lshw_multimedia_resources(record: &mut LshwMultimediaRecord, value: &str) {
    let mut memory = Vec::new();
    for part in value.split_whitespace() {
        let Some((kind, value)) = part.split_once(':') else {
            continue;
        };
        match kind {
            "irq" if record.irq.is_none() => record.irq = clean_lshw_multimedia_value(value),
            "memory" => {
                if let Some(value) = clean_lshw_multimedia_value(value) {
                    memory.push(value);
                }
            }
            _ => {}
        }
    }
    if !memory.is_empty() {
        record.memory_address = Some(memory.join("; "));
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

fn clean_hwinfo_value(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value.split('"').nth(1).unwrap_or(value).trim();
    if value.is_empty() || value.contains("unknown") {
        None
    } else {
        Some(value.to_string())
    }
}

fn clean_hwinfo_modules(value: &str) -> Vec<String> {
    let quoted = value
        .split('"')
        .enumerate()
        .filter_map(|(index, part)| (index % 2 == 1).then_some(part.trim()))
        .filter(|part| !part.is_empty() && !part.contains("unknown"))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if !quoted.is_empty() {
        return quoted;
    }

    value
        .split([',', ' '])
        .map(str::trim)
        .filter(|part| !part.is_empty() && !part.contains("unknown"))
        .map(ToString::to_string)
        .collect()
}

fn clean_hwinfo_pci_address(value: &str) -> Option<String> {
    clean_hwinfo_value(value).map(|value| {
        value
            .strip_prefix("pci@")
            .unwrap_or(value.as_str())
            .to_string()
    })
}

fn parse_hwinfo_sound_card_index(value: &str) -> Option<u32> {
    let re = Regex::new(r"card(\d+)").unwrap();
    re.captures(value).and_then(|caps| caps[1].parse().ok())
}
