use crate::ProbeContext;
use hw_model::BusInfo;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct SysfsPciRecord {
    pub path: PathBuf,
    pub address: String,
    pub vendor_id: Option<String>,
    pub device_id: Option<String>,
    pub class_id: Option<String>,
    pub subsystem_vendor_id: Option<String>,
    pub subsystem_device_id: Option<String>,
    pub driver: Option<String>,
    pub modules: Vec<String>,
}

pub(crate) async fn read_sysfs_pci_records(ctx: &ProbeContext<'_>) -> Vec<SysfsPciRecord> {
    let mut paths = ctx.runner.glob("/sys/bus/pci/devices/*").await.paths;
    paths.sort();

    let mut records = Vec::new();
    for path in paths {
        let Some(address) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_pci_address(address) {
            continue;
        }

        records.push(SysfsPciRecord {
            path: path.clone(),
            address: address.to_string(),
            vendor_id: read_pci_id(ctx, &path.join("vendor")).await,
            device_id: read_pci_id(ctx, &path.join("device")).await,
            class_id: read_pci_id(ctx, &path.join("class")).await,
            subsystem_vendor_id: read_pci_id(ctx, &path.join("subsystem_vendor")).await,
            subsystem_device_id: read_pci_id(ctx, &path.join("subsystem_device")).await,
            driver: read_uevent_value(ctx, &path.join("uevent"), "DRIVER").await,
            modules: read_kernel_modules(ctx, &path).await,
        });
    }

    records
}

pub(crate) fn is_pci_address(value: &str) -> bool {
    let mut parts = value.split([':', '.']);
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(domain), Some(bus), Some(device), Some(function), None)
            if is_hex_len(domain, 4)
                && is_hex_len(bus, 2)
                && is_hex_len(device, 2)
                && is_hex_len(function, 1)
    )
}

fn is_hex_len(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(crate) async fn read_kernel_modules(ctx: &ProbeContext<'_>, path: &Path) -> Vec<String> {
    let pattern = format!("{}/driver/module/drivers/*", path.display());
    let mut modules: Vec<_> = ctx
        .runner
        .glob(&pattern)
        .await
        .paths
        .into_iter()
        .filter_map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| {
                    name.rsplit_once(':')
                        .map(|(_, module)| module)
                        .or(Some(name))
                })
                .filter(|module| !module.trim().is_empty())
                .map(str::to_string)
        })
        .collect();
    modules.sort();
    modules.dedup();
    modules
}

pub(crate) fn pci_bus_from_uevent(uevent: &str) -> Option<BusInfo> {
    let address = parse_uevent_value_str(uevent, "PCI_SLOT_NAME")?.to_string();
    let (vendor_id, device_id) = parse_pci_id_pair(parse_uevent_value_str(uevent, "PCI_ID"));
    let (subsystem_vendor_id, subsystem_device_id) =
        parse_pci_id_pair(parse_uevent_value_str(uevent, "PCI_SUBSYS_ID"));
    let class = parse_uevent_value_str(uevent, "PCI_CLASS").and_then(|class| {
        normalize_pci_hex_id(class).map(|class| {
            if class.len() == 5 {
                format!("0{class}")
            } else {
                class
            }
        })
    });

    Some(BusInfo::Pci {
        address,
        vendor_id,
        device_id,
        subsystem_vendor_id,
        subsystem_device_id,
        class,
    })
}

fn parse_pci_id_pair(value: Option<&str>) -> (Option<String>, Option<String>) {
    let Some((vendor, device)) = value.and_then(|value| value.split_once(':')) else {
        return (None, None);
    };
    (normalize_pci_hex_id(vendor), normalize_pci_hex_id(device))
}

fn normalize_pci_hex_id(value: &str) -> Option<String> {
    let value = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    (!value.is_empty() && value.chars().all(|ch| ch.is_ascii_hexdigit()))
        .then(|| value.to_ascii_lowercase())
}

async fn read_pci_id(ctx: &ProbeContext<'_>, path: &Path) -> Option<String> {
    let result = ctx.runner.read_file(path).await;
    if !result.is_success() {
        return None;
    }
    let value = result.stdout.trim().trim_start_matches("0x");
    (!value.is_empty()).then(|| value.to_ascii_lowercase())
}

async fn read_uevent_value(ctx: &ProbeContext<'_>, path: &Path, key: &str) -> Option<String> {
    let result = ctx.runner.read_file(path).await;
    if !result.is_success() {
        return None;
    }
    parse_uevent_value_str(&result.stdout, key).map(str::to_string)
}

fn parse_uevent_value_str<'a>(input: &'a str, key: &str) -> Option<&'a str> {
    input.lines().find_map(|line| {
        let (candidate, value) = line.split_once('=')?;
        (candidate == key && !value.trim().is_empty()).then(|| value.trim())
    })
}
