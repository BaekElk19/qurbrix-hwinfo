use crate::util::split_csv_words;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PciRecord {
    pub address: String,
    pub class_name: Option<String>,
    pub class_id: Option<String>,
    pub vendor: Option<String>,
    pub vendor_id: Option<String>,
    pub device: Option<String>,
    pub device_id: Option<String>,
    pub subsystem: Option<String>,
    pub subsystem_vendor_id: Option<String>,
    pub subsystem_device_id: Option<String>,
    pub kernel_driver: Option<String>,
    pub kernel_modules: Vec<String>,
}

pub fn parse_lspci_nn_k(input: &str) -> Vec<PciRecord> {
    let header = Regex::new(r"^(?P<addr>[0-9a-fA-F:.]+)\s+(?P<class>.+?)\s+\[(?P<class_id>[0-9a-fA-F]{4})\]:\s+(?P<description>.+?)\s+\[(?P<vendor_id>[0-9a-fA-F]{4}):(?P<device_id>[0-9a-fA-F]{4})\](?:\s+\(rev .+\))?$").unwrap();
    let subsystem =
        Regex::new(r"^\s*Subsystem:\s*(?P<subsystem>.*?)(?:\s+\[(?P<sub_vendor>[0-9a-fA-F]{4}):(?P<sub_device>[0-9a-fA-F]{4})\])?$")
            .unwrap();
    let driver = Regex::new(r"^\s*Kernel driver in use:\s*(?P<driver>.+)$").unwrap();
    let modules = Regex::new(r"^\s*Kernel modules:\s*(?P<modules>.+)$").unwrap();

    let mut records = Vec::new();
    let mut current: Option<PciRecord> = None;

    for line in input.lines() {
        if let Some(caps) = header.captures(line) {
            if let Some(record) = current.take() {
                records.push(record);
            }
            let mut address = caps["addr"].to_string();
            if !address.contains(':') || address.matches(':').count() == 1 {
                address = format!("0000:{address}");
            }
            current = Some(PciRecord {
                address,
                class_name: Some(caps["class"].trim().to_string()),
                class_id: Some(caps["class_id"].to_ascii_lowercase()),
                device: Some(caps["description"].trim().to_string()),
                vendor_id: Some(caps["vendor_id"].to_ascii_lowercase()),
                device_id: Some(caps["device_id"].to_ascii_lowercase()),
                ..Default::default()
            });
            continue;
        }

        let Some(record) = current.as_mut() else {
            continue;
        };
        if let Some(caps) = subsystem.captures(line) {
            let subsystem = caps.name("subsystem").map(|value| value.as_str().trim());
            if let Some(subsystem) = subsystem.filter(|value| !value.is_empty()) {
                record.subsystem = Some(subsystem.to_string());
            }
            if let Some(sub_vendor) = caps.name("sub_vendor") {
                record.subsystem_vendor_id = Some(sub_vendor.as_str().to_ascii_lowercase());
            }
            if let Some(sub_device) = caps.name("sub_device") {
                record.subsystem_device_id = Some(sub_device.as_str().to_ascii_lowercase());
            }
        } else if let Some(caps) = driver.captures(line) {
            record.kernel_driver = Some(caps["driver"].trim().to_string());
        } else if let Some(caps) = modules.captures(line) {
            record.kernel_modules = split_csv_words(&caps["modules"]);
        }
    }

    if let Some(record) = current.take() {
        records.push(record);
    }

    records
}

pub fn parse_lspci_host_bridge_chipset(input: &str) -> Option<String> {
    let records = parse_lspci_nn_k(input);
    if let Some(chipset) = records
        .iter()
        .find(|record| is_isa_bridge(record))
        .and_then(pci_subsystem_display)
    {
        return Some(chipset);
    }

    records
        .iter()
        .find(|record| record.class_id.as_deref() == Some("0600"))
        .and_then(pci_description_display)
}

fn is_isa_bridge(record: &PciRecord) -> bool {
    record.class_id.as_deref() == Some("0601")
        || record
            .class_name
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case("ISA bridge"))
}

fn pci_subsystem_display(record: &PciRecord) -> Option<String> {
    let subsystem = record.subsystem.as_deref()?;
    let ids = match (
        record.subsystem_vendor_id.as_deref(),
        record.subsystem_device_id.as_deref(),
    ) {
        (Some(vendor_id), Some(device_id)) => format!(" [{vendor_id}:{device_id}]"),
        _ => String::new(),
    };
    Some(format!("{subsystem}{ids}"))
}

fn pci_description_display(record: &PciRecord) -> Option<String> {
    let description = record.device.as_deref()?;
    let ids = match (record.vendor_id.as_deref(), record.device_id.as_deref()) {
        (Some(vendor_id), Some(device_id)) => format!(" [{vendor_id}:{device_id}]"),
        _ => String::new(),
    };
    Some(format!("{description}{ids}"))
}
