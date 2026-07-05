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
    pub subsystem_vendor_id: Option<String>,
    pub subsystem_device_id: Option<String>,
    pub kernel_driver: Option<String>,
    pub kernel_modules: Vec<String>,
}

pub fn parse_lspci_nn_k(input: &str) -> Vec<PciRecord> {
    let header = Regex::new(r"^(?P<addr>[0-9a-fA-F:.]+)\s+(?P<class>.+?)\s+\[(?P<class_id>[0-9a-fA-F]{4})\]:\s+(?P<description>.+?)\s+\[(?P<vendor_id>[0-9a-fA-F]{4}):(?P<device_id>[0-9a-fA-F]{4})\](?:\s+\(rev .+\))?$").unwrap();
    let subsystem = Regex::new(
        r"^\s*Subsystem:.*\[(?P<sub_vendor>[0-9a-fA-F]{4}):(?P<sub_device>[0-9a-fA-F]{4})\]",
    )
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
            record.subsystem_vendor_id = Some(caps["sub_vendor"].to_ascii_lowercase());
            record.subsystem_device_id = Some(caps["sub_device"].to_ascii_lowercase());
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
