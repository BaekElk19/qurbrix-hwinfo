use crate::error::Result;
use hw_model::{
    BusInfo, CoreIdentityGroup, Device, DeviceProperties, IdentityCoverage, QuickProbeReport,
    BINDID_V2_ALGORITHM, FINGERPRINT_VERSION, SNAPSHOT_SCHEMA_VERSION,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub fn canonicalize_devices(
    devices: &[Device],
    warnings: Vec<String>,
    trusted_absent: BTreeSet<CoreIdentityGroup>,
    observed_at: String,
) -> Result<QuickProbeReport> {
    let mut identity_records = Vec::new();
    let mut configuration_records = Vec::new();

    for device in devices {
        collect_device_records(device, &mut identity_records, &mut configuration_records);
    }
    for group in &trusted_absent {
        identity_records.push(format!("absence:group={}", group_name(*group)));
    }
    normalize_records(&mut identity_records);
    normalize_records(&mut configuration_records);

    let covered = covered_groups(&identity_records);
    let missing = CoreIdentityGroup::REQUIRED
        .iter()
        .filter(|group| !covered.contains(group) && !trusted_absent.contains(group))
        .copied()
        .collect::<Vec<_>>();
    let machine_payload = serde_json::to_vec(&identity_records)?;
    let machine_bind_id = sha256_hex(&machine_payload);
    let configuration_payload = BTreeMap::from([
        (
            "configuration_records",
            serde_json::to_value(&configuration_records)?,
        ),
        (
            "fingerprint_version",
            serde_json::to_value(FINGERPRINT_VERSION)?,
        ),
        ("identity_records", serde_json::to_value(&identity_records)?),
    ]);
    let configuration_bytes = serde_json::to_vec(&configuration_payload)?;
    let configuration_fingerprint = sha256_hex(&configuration_bytes);

    Ok(QuickProbeReport {
        schema_version: SNAPSHOT_SCHEMA_VERSION.to_string(),
        fingerprint_version: FINGERPRINT_VERSION,
        bindid_algorithm: BINDID_V2_ALGORITHM.to_string(),
        machine_bind_id,
        configuration_fingerprint: configuration_fingerprint.clone(),
        canonical_payload_sha256: configuration_fingerprint,
        observed_at,
        identity_records,
        configuration_records,
        coverage: IdentityCoverage {
            covered: covered.into_iter().collect(),
            missing,
            trusted_absent: trusted_absent.into_iter().collect(),
        },
        warnings,
    })
}

fn collect_device_records(
    device: &Device,
    identity_records: &mut Vec<String>,
    configuration_records: &mut Vec<String>,
) {
    match &device.properties {
        DeviceProperties::System(info) => {
            push_record(
                identity_records,
                "platform",
                &[
                    ("uuid", info.uuid.as_deref()),
                    (
                        "serial",
                        info.serial.as_deref().or(device.serial.as_deref()),
                    ),
                    (
                        "manufacturer",
                        info.manufacturer.as_deref().or(device.vendor.as_deref()),
                    ),
                    (
                        "product",
                        info.product_name.as_deref().or(device.model.as_deref()),
                    ),
                ],
            );
            push_record(
                configuration_records,
                "system",
                &[
                    ("kernel", info.kernel.as_deref()),
                    ("os", info.os.as_deref()),
                    ("architecture", info.architecture.as_deref()),
                ],
            );
        }
        DeviceProperties::Motherboard(info) => push_record(
            identity_records,
            "platform",
            &[
                ("board_serial", info.serial.as_deref()),
                ("board_manufacturer", info.manufacturer.as_deref()),
                ("board_product", info.product_name.as_deref()),
            ],
        ),
        DeviceProperties::Bios(info) => push_record(
            configuration_records,
            "bios",
            &[
                ("vendor", info.vendor.as_deref()),
                ("version", info.version.as_deref()),
                ("release_date", info.release_date.as_deref()),
                ("firmware_revision", info.firmware_revision.as_deref()),
            ],
        ),
        DeviceProperties::Cpu(info) => {
            let sockets = info.sockets.map(|value| value.to_string());
            let cores = info.cores.map(|value| value.to_string());
            let threads = info.threads.map(|value| value.to_string());
            let socket_designations = joined(&info.socket_designations);
            let serials = joined(&info.serial_numbers);
            push_record(
                identity_records,
                "cpu",
                &[
                    (
                        "vendor",
                        info.vendor.as_deref().or(device.vendor.as_deref()),
                    ),
                    (
                        "model",
                        info.name
                            .as_deref()
                            .or(info.model.as_deref())
                            .or(device.model.as_deref()),
                    ),
                    ("implementer", info.cpu_implementer.as_deref()),
                    ("part", info.cpu_part.as_deref()),
                    ("architecture", info.architecture.as_deref()),
                    ("sockets", sockets.as_deref()),
                    ("cores", cores.as_deref()),
                    ("threads", threads.as_deref()),
                    ("socket_designations", socket_designations.as_deref()),
                    ("serials", serials.as_deref()),
                ],
            );
        }
        DeviceProperties::Memory(info) => {
            let size = info.size_bytes.map(|value| value.to_string());
            push_record(
                identity_records,
                "memory",
                &[
                    (
                        "serial",
                        info.serial.as_deref().or(device.serial.as_deref()),
                    ),
                    (
                        "part",
                        info.part_number.as_deref().or(device.model.as_deref()),
                    ),
                    ("locator", info.locator.as_deref()),
                    ("bank", info.bank_locator.as_deref()),
                    ("size_bytes", size.as_deref()),
                ],
            );
            push_record(
                configuration_records,
                "memory_firmware",
                &[
                    (
                        "identity",
                        info.serial.as_deref().or(info.locator.as_deref()),
                    ),
                    ("version", info.firmware_version.as_deref()),
                ],
            );
        }
        DeviceProperties::Storage(info) => {
            let bus = stable_bus_identity(device.bus.as_ref());
            push_record(
                identity_records,
                "storage",
                &[
                    ("wwn", info.wwn.as_deref()),
                    ("serial", device.serial.as_deref()),
                    (
                        "model",
                        device.model.as_deref().or(info.controller_model.as_deref()),
                    ),
                    ("bus", bus.as_deref()),
                ],
            );
            push_record(
                configuration_records,
                "storage_firmware",
                &[
                    ("identity", info.wwn.as_deref().or(device.serial.as_deref())),
                    ("firmware", info.firmware.as_deref()),
                    (
                        "driver",
                        device
                            .driver
                            .as_ref()
                            .and_then(|driver| driver.name.as_deref()),
                    ),
                    (
                        "driver_version",
                        device
                            .driver
                            .as_ref()
                            .and_then(|driver| driver.version.as_deref()),
                    ),
                ],
            );
        }
        DeviceProperties::Network(info)
            if is_physical_network(device, info.interface.as_deref()) =>
        {
            let mac = info.mac.as_deref().and_then(normalize_physical_mac);
            let bus = stable_bus_identity(device.bus.as_ref());
            push_record(
                identity_records,
                "physical_network",
                &[("mac", mac.as_deref()), ("bus", bus.as_deref())],
            );
            push_record(
                configuration_records,
                "network_firmware",
                &[
                    ("mac", mac.as_deref()),
                    ("firmware", info.firmware.as_deref()),
                    (
                        "driver",
                        device
                            .driver
                            .as_ref()
                            .and_then(|driver| driver.name.as_deref()),
                    ),
                    ("driver_version", info.driver_version.as_deref()),
                ],
            );
        }
        DeviceProperties::Gpu(info) if !is_software_renderer(device, info.renderer.as_deref()) => {
            let pci = pci_identity(device.bus.as_ref());
            push_record(
                identity_records,
                "gpu",
                &[
                    ("pci", pci.as_deref()),
                    (
                        "vendor",
                        device.vendor.as_deref().or(info.vendor.as_deref()),
                    ),
                    (
                        "model",
                        device.model.as_deref().or(info.description.as_deref()),
                    ),
                ],
            );
            push_driver_configuration(device, configuration_records, "gpu_driver");
        }
        _ => {}
    }
    if !matches!(
        device.properties,
        DeviceProperties::Storage(_) | DeviceProperties::Network(_) | DeviceProperties::Gpu(_)
    ) {
        push_driver_configuration(device, configuration_records, "device_driver");
    }
}

fn push_driver_configuration(device: &Device, output: &mut Vec<String>, kind: &str) {
    let Some(driver) = &device.driver else {
        return;
    };
    let identity =
        stable_bus_identity(device.bus.as_ref()).or_else(|| normalize(device.id.as_str()));
    let modules = joined(&driver.modules);
    push_record(
        output,
        kind,
        &[
            ("identity", identity.as_deref()),
            ("name", driver.name.as_deref()),
            ("version", driver.version.as_deref()),
            ("modules", modules.as_deref()),
        ],
    );
}

fn push_record(output: &mut Vec<String>, kind: &str, fields: &[(&str, Option<&str>)]) {
    let mut fields = fields
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_ref()
                .and_then(|value| normalize(value))
                .map(|value| ((*key).to_string(), value))
        })
        .collect::<Vec<_>>();
    fields.sort_by(|left, right| left.0.cmp(&right.0));
    if fields.is_empty() {
        return;
    }
    output.push(format!(
        "{kind}:{}",
        fields
            .into_iter()
            .map(|(key, value)| format!("{key}={}", escape(&value)))
            .collect::<Vec<_>>()
            .join("|")
    ));
}

fn normalize(value: &str) -> Option<String> {
    const PLACEHOLDERS: &[&str] = &[
        "none",
        "n/a",
        "not specified",
        "no asset tag",
        "to be filled by o.e.m.",
        "system serial number",
        "default string",
        "unknown",
    ];
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty()
        || PLACEHOLDERS
            .iter()
            .any(|placeholder| value.eq_ignore_ascii_case(placeholder))
    {
        None
    } else {
        Some(value.to_ascii_lowercase())
    }
}

fn normalize_mac(value: &str) -> Option<String> {
    let mac = normalize(value)?.replace('-', ":");
    let parts = mac.split(':').collect::<Vec<_>>();
    (parts.len() == 6
        && parts.iter().all(|part| {
            part.len() == 2 && part.chars().all(|character| character.is_ascii_hexdigit())
        })
        && mac != "00:00:00:00:00:00")
        .then_some(mac)
}

fn normalize_physical_mac(value: &str) -> Option<String> {
    let mac = normalize_mac(value)?;
    let first_octet = u8::from_str_radix(&mac[..2], 16).ok()?;
    (first_octet & 0x02 == 0).then_some(mac)
}

fn joined(values: &[String]) -> Option<String> {
    let mut normalized = values
        .iter()
        .filter_map(|value| normalize(value))
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    (!normalized.is_empty()).then(|| normalized.join(","))
}

fn escape(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('|', "%7c")
        .replace('=', "%3d")
}

fn normalize_records(records: &mut Vec<String>) {
    records.sort();
    records.dedup();
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn covered_groups(records: &[String]) -> BTreeSet<CoreIdentityGroup> {
    records
        .iter()
        .filter_map(|record| {
            let kind = record.split_once(':')?.0;
            match kind {
                "platform" => Some(CoreIdentityGroup::Platform),
                "cpu" => Some(CoreIdentityGroup::Cpu),
                "memory" => Some(CoreIdentityGroup::Memory),
                "storage" => Some(CoreIdentityGroup::Storage),
                "physical_network" => Some(CoreIdentityGroup::PhysicalNetwork),
                "gpu" => Some(CoreIdentityGroup::Gpu),
                _ => None,
            }
        })
        .collect()
}

fn group_name(group: CoreIdentityGroup) -> &'static str {
    match group {
        CoreIdentityGroup::Platform => "platform",
        CoreIdentityGroup::Cpu => "cpu",
        CoreIdentityGroup::Memory => "memory",
        CoreIdentityGroup::Storage => "storage",
        CoreIdentityGroup::PhysicalNetwork => "physical_network",
        CoreIdentityGroup::Gpu => "gpu",
    }
}

fn is_physical_network(device: &Device, interface: Option<&str>) -> bool {
    if matches!(device.bus, Some(BusInfo::Virtual)) {
        return false;
    }
    let interface = interface.unwrap_or_default().to_ascii_lowercase();
    !interface.is_empty()
        && interface != "lo"
        && ![
            "docker",
            "veth",
            "virbr",
            "br-",
            "tun",
            "tap",
            "wg",
            "tailscale",
            "zt",
        ]
        .iter()
        .any(|prefix| interface.starts_with(prefix))
}

fn is_software_renderer(device: &Device, renderer: Option<&str>) -> bool {
    [
        Some(device.name.as_str()),
        device.model.as_deref(),
        renderer,
    ]
    .into_iter()
    .flatten()
    .map(str::to_ascii_lowercase)
    .any(|value| {
        ["llvmpipe", "softpipe", "software rasterizer", "swiftshader"]
            .iter()
            .any(|marker| value.contains(marker))
    })
}

fn stable_bus_identity(bus: Option<&BusInfo>) -> Option<String> {
    match bus {
        Some(BusInfo::Pci { address, .. }) => {
            normalize(address).map(|value| format!("pci:{value}"))
        }
        Some(BusInfo::Platform { path }) => {
            normalize(path).map(|value| format!("platform:{value}"))
        }
        _ => None,
    }
}

fn pci_identity(bus: Option<&BusInfo>) -> Option<String> {
    let Some(BusInfo::Pci {
        vendor_id,
        device_id,
        subsystem_vendor_id,
        subsystem_device_id,
        ..
    }) = bus
    else {
        return None;
    };
    let value = Value::Array(
        [
            vendor_id,
            device_id,
            subsystem_vendor_id,
            subsystem_device_id,
        ]
        .into_iter()
        .map(|value| {
            value
                .as_deref()
                .and_then(normalize)
                .map(Value::String)
                .unwrap_or(Value::Null)
        })
        .collect(),
    );
    Some(value.to_string())
}
