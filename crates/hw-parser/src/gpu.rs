use crate::{parse_lspci_nn_k, PciRecord};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LshwDisplayRecord {
    pub product: Option<String>,
    pub vendor: Option<String>,
    pub bus_info: Option<String>,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DmesgGpuVramRecord {
    pub pci_address: String,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GlxinfoBasicRecord {
    pub renderer: Option<String>,
    pub vendor: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NvidiaSmiMemoryRecord {
    pub pci_address: String,
    pub memory_bytes: u64,
}

pub fn parse_gpu_lspci(input: &str) -> Vec<PciRecord> {
    parse_lspci_nn_k(input)
        .into_iter()
        .filter(|record| {
            let class = record
                .class_name
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase();
            class.contains("vga") || class.contains("3d controller") || class.contains("display")
        })
        .collect()
}

pub fn parse_glxinfo_basic(input: &str) -> GlxinfoBasicRecord {
    let mut record = GlxinfoBasicRecord::default();

    for line in input.lines() {
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let value = clean_lshw_display_value(value);
        match key.trim() {
            "OpenGL renderer string" => record.renderer = value,
            "OpenGL vendor string" => record.vendor = value,
            "OpenGL version string" => record.version = value,
            "OpenGL core profile version string" if record.version.is_none() => {
                record.version = value;
            }
            _ => {}
        }
    }

    record
}

pub fn parse_dmesg_gpu_vram(input: &str) -> Vec<DmesgGpuVramRecord> {
    input
        .lines()
        .filter_map(|line| {
            let pci_address = line
                .split_whitespace()
                .map(clean_dmesg_token)
                .find(|token| is_pci_address(token))?;
            let memory_bytes = parse_vram_mb(line)?.checked_mul(1024 * 1024)?;
            Some(DmesgGpuVramRecord {
                pci_address: pci_address.to_string(),
                memory_bytes,
            })
        })
        .collect()
}

pub fn parse_nvidia_smi_memory_csv(input: &str) -> Vec<NvidiaSmiMemoryRecord> {
    input
        .lines()
        .filter_map(|line| {
            let (address, memory) = line.split_once(',')?;
            let pci_address = normalize_nvidia_pci_address(address)?;
            let memory_mib = memory
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<u64>().ok())?;
            let memory_bytes = memory_mib.checked_mul(1024 * 1024)?;
            Some(NvidiaSmiMemoryRecord {
                pci_address,
                memory_bytes,
            })
        })
        .collect()
}

pub fn parse_nvidia_settings_videoram(input: &str) -> Option<u64> {
    let mut values = input.lines().filter_map(|line| {
        if !line.contains("VideoRam") {
            return None;
        }
        let (_, value) = line.rsplit_once(':')?;
        let digits = value
            .trim()
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        digits.parse::<u64>().ok()?.checked_mul(1024)
    });
    let memory_bytes = values.next()?;
    values.next().is_none().then_some(memory_bytes)
}

pub fn parse_lshw_display(input: &str) -> Vec<LshwDisplayRecord> {
    let mut records = Vec::new();
    let mut current: Option<LshwDisplayRecord> = None;

    for line in input.lines().chain(std::iter::once("")) {
        let trimmed = line.trim();
        if trimmed.starts_with("*-display") {
            push_lshw_display_record(&mut records, current.take());
            current = Some(LshwDisplayRecord::default());
            continue;
        }
        if trimmed.starts_with("*-") {
            push_lshw_display_record(&mut records, current.take());
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
            "product" => record.product = clean_lshw_display_value(value),
            "vendor" => record.vendor = clean_lshw_display_value(value),
            "bus info" => record.bus_info = clean_lshw_display_value(value),
            "configuration" => parse_lshw_display_configuration(record, value),
            _ => {}
        }
    }

    push_lshw_display_record(&mut records, current.take());
    records
}

fn clean_dmesg_token(value: &str) -> &str {
    value
        .trim_matches(|ch: char| !ch.is_ascii_hexdigit() && ch != ':' && ch != '.')
        .trim_end_matches(':')
}

fn is_pci_address(value: &str) -> bool {
    value.len() == 12
        && value.as_bytes().get(4) == Some(&b':')
        && value.as_bytes().get(7) == Some(&b':')
        && value.as_bytes().get(10) == Some(&b'.')
        && value
            .chars()
            .enumerate()
            .all(|(index, ch)| matches!(index, 4 | 7 | 10) || ch.is_ascii_hexdigit())
}

fn normalize_nvidia_pci_address(value: &str) -> Option<String> {
    let value = value.trim();
    let mut parts = value.split(':');
    let domain = parts.next()?;
    let bus = parts.next()?;
    let slot_function = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let (slot, function) = slot_function.split_once('.')?;
    if domain.is_empty()
        || domain.len() > 8
        || !domain.chars().all(|ch| ch.is_ascii_hexdigit())
        || bus.len() != 2
        || !bus.chars().all(|ch| ch.is_ascii_hexdigit())
        || slot.len() != 2
        || !slot.chars().all(|ch| ch.is_ascii_hexdigit())
        || function.len() != 1
        || !function.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return None;
    }
    let domain = if domain.len() > 4 {
        &domain[domain.len() - 4..]
    } else {
        domain
    };
    Some(format!(
        "{:0>4}:{}:{}.{}",
        domain.to_ascii_lowercase(),
        bus.to_ascii_lowercase(),
        slot.to_ascii_lowercase(),
        function.to_ascii_lowercase()
    ))
}

fn parse_vram_mb(line: &str) -> Option<u64> {
    let (_, tail) = line.split_once("VRAM")?;
    let digits = tail
        .trim_start_matches(|ch: char| !ch.is_ascii_digit())
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

fn push_lshw_display_record(
    records: &mut Vec<LshwDisplayRecord>,
    record: Option<LshwDisplayRecord>,
) {
    if let Some(record) = record {
        if record.product.is_some() || record.vendor.is_some() || record.bus_info.is_some() {
            records.push(record);
        }
    }
}

fn parse_lshw_display_configuration(record: &mut LshwDisplayRecord, value: &str) {
    for part in value.split_whitespace() {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key == "driver" {
            record.driver = clean_lshw_display_value(value);
        }
    }
}

fn clean_lshw_display_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("n/a") || value == "(none)" {
        None
    } else {
        Some(value.to_string())
    }
}
