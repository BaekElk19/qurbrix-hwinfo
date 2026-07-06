use crate::{
    sysfs_pci::{pci_bus_from_uevent, read_kernel_modules, read_sysfs_pci_records},
    Probe, ProbeContext, ProbeResult,
};
use async_trait::async_trait;
use hw_model::{
    device_id, BiosInfo, BusInfo, CpuInfo, Device, DeviceKind, DeviceProperties, DeviceRef,
    DriverInfo, DriverStatus, GpuInfo, MemoryInfo, MonitorInfo, MotherboardInfo, NetworkInfo,
    ScanWarning, SourceEvidence, SourceKind, SourceStatus, StorageInfo,
};
use hw_parser::{
    infer_cpu_vendor_from_name, lookup_pnp_manufacturer, merge_cpu_records, normalize_arch,
    normalize_cpu_vendor_id, normalize_gpu_vendor, normalize_gpu_vendor_id,
    parse_dmidecode_bios_board, parse_dmidecode_memory, parse_dmidecode_processor, parse_edid,
    parse_gpu_lspci, parse_ip_j_addr_result, parse_ip_j_link_result, parse_lsblk_json_result,
    parse_lscpu, parse_lshw_disk, parse_lshw_display, parse_lshw_memory, parse_lshw_network,
    parse_lshw_processor, parse_proc_cpuinfo, parse_proc_hardware, parse_proc_meminfo_total_bytes,
    parse_size_to_bytes, parse_smartctl_json, parse_speed_mtps, parse_xrandr_query,
    parse_xrandr_verbose, DmiBiosBoardRecord, DmiMemoryRecord, LshwDiskRecord, LshwDisplayRecord,
    LshwNetworkRecord,
};
use hw_source::{CommandSpec, SourceBytesResult, SourceErrorKind};
use std::{collections::HashMap, path::Path};

pub struct CpuProbe;
pub struct NetworkProbe;
pub struct StorageProbe;
pub struct MemoryProbe;
pub struct BiosProbe;
pub struct GpuProbe;
pub struct MonitorProbe;

#[async_trait]
impl Probe for CpuProbe {
    fn name(&self) -> &'static str {
        "cpu"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Cpu]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let lscpu_result = ctx
            .runner
            .run_command(
                &CommandSpec::new("lscpu", std::iter::empty::<&str>()),
                ctx.timeout,
            )
            .await;
        let lshw_result = ctx
            .runner
            .run_command(
                &CommandSpec::new("lshw", ["-class", "processor"]),
                ctx.timeout,
            )
            .await;
        let dmi_result = ctx
            .runner
            .run_command(&CommandSpec::new("dmidecode", ["-t", "4"]), ctx.timeout)
            .await;
        let proc_cpuinfo_result = ctx.runner.read_file(Path::new("/proc/cpuinfo")).await;
        let proc_hardware_result = ctx.runner.read_file(Path::new("/proc/hardware")).await;

        let mut warnings = Vec::new();
        let lscpu = if lscpu_result.is_success() {
            let record = parse_lscpu(&lscpu_result.stdout);
            (!record.is_empty()).then_some(record)
        } else {
            warnings.extend(ProbeResult::source_failure(self.name(), &lscpu_result).warnings);
            None
        };
        let lshw = if lshw_result.is_success() {
            let record = parse_lshw_processor(&lshw_result.stdout);
            (!record.is_empty()).then_some(record)
        } else {
            warnings.extend(ProbeResult::source_failure(self.name(), &lshw_result).warnings);
            None
        };
        let dmi = if dmi_result.is_success() {
            parse_dmidecode_processor(&dmi_result.stdout)
                .into_iter()
                .filter(|record| record.is_useful())
                .collect()
        } else {
            warnings.extend(ProbeResult::source_failure(self.name(), &dmi_result).warnings);
            Vec::new()
        };
        let proc_cpuinfo = if proc_cpuinfo_result.is_success() {
            let record = parse_proc_cpuinfo(&proc_cpuinfo_result.stdout);
            (!record.is_empty()).then_some(record)
        } else {
            None
        };
        let proc_hardware = if proc_hardware_result.is_success() {
            let record = parse_proc_hardware(&proc_hardware_result.stdout);
            (!record.is_empty()).then_some(record)
        } else {
            None
        };
        let lscpu_contributed = lscpu.is_some();
        let lshw_contributed = lshw.is_some();
        let dmi_contributed = !dmi.is_empty();
        let proc_cpuinfo_contributed = proc_cpuinfo.is_some();
        let proc_hardware_contributed = proc_hardware.is_some();
        if lscpu.is_none()
            && lshw.is_none()
            && dmi.is_empty()
            && proc_cpuinfo.is_none()
            && proc_hardware.is_none()
        {
            if !proc_cpuinfo_result.is_success() {
                warnings.extend(
                    ProbeResult::source_failure(self.name(), &proc_cpuinfo_result).warnings,
                );
            }
            return ProbeResult {
                devices: Vec::new(),
                warnings,
                consumed: Vec::new(),
            };
        }
        let lscpu_vendor = lscpu.as_ref().and_then(|record| record.vendor.clone());
        let lshw_vendor = lshw.as_ref().and_then(|record| record.vendor.clone());
        let merged = merge_cpu_records(
            merge_cpu_record_fallback(
                merge_cpu_record_fallback(lscpu, proc_cpuinfo),
                proc_hardware,
            ),
            lshw,
            &dmi,
        );
        let architecture = merged
            .architecture
            .as_deref()
            .and_then(normalize_arch)
            .map(str::to_string)
            .or_else(|| merged.architecture.clone());
        let vendor = merged
            .vendor
            .as_deref()
            .and_then(normalize_cpu_vendor_id)
            .map(str::to_string)
            .or_else(|| {
                lscpu_vendor
                    .as_deref()
                    .and_then(normalize_cpu_vendor_id)
                    .map(str::to_string)
            })
            .or_else(|| {
                lshw_vendor
                    .as_deref()
                    .and_then(normalize_cpu_vendor_id)
                    .map(str::to_string)
            })
            .or_else(|| {
                merged
                    .name
                    .as_deref()
                    .and_then(infer_cpu_vendor_from_name)
                    .map(str::to_string)
            })
            .or_else(|| merged.vendor.clone());
        let mut device = Device::new(
            "cpu:0",
            DeviceKind::Cpu,
            merged.name.clone().unwrap_or_else(|| "CPU".to_string()),
            DeviceProperties::Cpu(CpuInfo {
                name: merged.name,
                vendor,
                architecture,
                cores: merged.cores,
                threads: merged.threads,
                sockets: merged.sockets,
                max_freq_mhz: merged.max_freq_mhz,
                min_freq_mhz: merged.min_freq_mhz,
                current_freq_mhz: merged.current_freq_mhz,
                family: merged.family,
                model: merged.model,
                stepping: merged.stepping,
                bogomips: merged.bogomips,
                virtualization: merged.virtualization,
                flags: merged.flags,
            }),
        );
        for (source, contributed) in [
            (&lscpu_result, lscpu_contributed),
            (&lshw_result, lshw_contributed),
            (&dmi_result, dmi_contributed),
        ] {
            if contributed {
                device = device.with_source(SourceEvidence {
                    source: source.source.clone(),
                    kind: SourceKind::Command,
                    status: SourceStatus::Success,
                    summary: None,
                });
            }
        }
        if proc_cpuinfo_contributed {
            device = device.with_source(SourceEvidence {
                source: proc_cpuinfo_result.source,
                kind: SourceKind::Procfs,
                status: SourceStatus::Success,
                summary: None,
            });
        }
        if proc_hardware_contributed {
            device = device.with_source(SourceEvidence {
                source: proc_hardware_result.source,
                kind: SourceKind::Procfs,
                status: SourceStatus::Success,
                summary: None,
            });
        }
        ProbeResult {
            devices: vec![device],
            warnings,
            consumed: Vec::new(),
        }
    }
}

fn merge_cpu_record_fallback(
    primary: Option<hw_parser::CpuRecord>,
    fallback: Option<hw_parser::CpuRecord>,
) -> Option<hw_parser::CpuRecord> {
    let Some(fallback) = fallback else {
        return primary;
    };
    let Some(mut primary) = primary else {
        return Some(fallback);
    };

    primary.architecture = primary.architecture.or(fallback.architecture);
    primary.threads = primary.threads.or(fallback.threads);
    primary.model_name = primary.model_name.or(fallback.model_name);
    primary.vendor = primary.vendor.or(fallback.vendor);
    primary.cores_per_socket = primary.cores_per_socket.or(fallback.cores_per_socket);
    primary.sockets = primary.sockets.or(fallback.sockets);
    primary.cpu_mhz = primary.cpu_mhz.or(fallback.cpu_mhz);
    primary.cpu_max_mhz = primary.cpu_max_mhz.or(fallback.cpu_max_mhz);
    primary.cpu_min_mhz = primary.cpu_min_mhz.or(fallback.cpu_min_mhz);
    primary.cpu_family = primary.cpu_family.or(fallback.cpu_family);
    primary.cpu_model = primary.cpu_model.or(fallback.cpu_model);
    primary.stepping = primary.stepping.or(fallback.stepping);
    primary.bogomips = primary.bogomips.or(fallback.bogomips);
    if primary.flags.is_empty() {
        primary.flags = fallback.flags;
    }
    primary.virtualization = primary.virtualization.or(fallback.virtualization);

    Some(primary)
}

#[async_trait]
impl Probe for NetworkProbe {
    fn name(&self) -> &'static str {
        "network"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Network]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx
            .runner
            .run_command(&CommandSpec::new("ip", ["-j", "link"]), ctx.timeout)
            .await;
        if !result.is_success() {
            let mut probe_result = ProbeResult::source_failure(self.name(), &result);
            probe_result.devices = network_devices_from_sysfs(ctx).await;
            return probe_result;
        }
        let records = match parse_ip_j_link_result(&result.stdout) {
            Ok(records) => records,
            Err(err) => {
                return ProbeResult {
                    devices: Vec::new(),
                    warnings: vec![ScanWarning::new(
                        "parse_failed",
                        format!(
                            "network source '{}' could not be parsed: {err}",
                            result.source
                        ),
                    )
                    .with_source(result.source)],
                    consumed: Vec::new(),
                };
            }
        };
        let mut warnings = Vec::new();
        let addresses = network_ip_addresses(ctx, self.name(), &mut warnings).await;
        let lshw = network_lshw_records(ctx).await;
        let mut devices = Vec::new();
        for net in records {
            if is_ignored_network_interface(&net.ifname) {
                continue;
            }
            let enrichment = network_sysfs_enrichment(ctx, &net.ifname).await;
            let address_info = addresses
                .by_interface
                .get(&net.ifname)
                .cloned()
                .unwrap_or_default();
            let has_ip_addresses = !address_info.ipv4.is_empty() || !address_info.ipv6.is_empty();
            let mut device = Device::new(
                device_id::network(net.address.as_deref(), &net.ifname),
                DeviceKind::Network,
                net.ifname.clone(),
                DeviceProperties::Network(NetworkInfo {
                    interface: Some(net.ifname.clone()),
                    network_type: enrichment.network_type(),
                    mac: net.address,
                    operstate: net.operstate,
                    speed_mbps: enrichment.speed_mbps,
                    duplex: enrichment.duplex.clone(),
                    ipv4: address_info.ipv4,
                    ipv6: address_info.ipv6,
                    ..Default::default()
                }),
            )
            .with_source(SourceEvidence {
                source: result.source.clone(),
                kind: SourceKind::Command,
                status: SourceStatus::Success,
                summary: None,
            });
            if has_ip_addresses {
                device = device.with_source(SourceEvidence {
                    source: addresses.source.clone(),
                    kind: SourceKind::Command,
                    status: SourceStatus::Success,
                    summary: None,
                });
            }
            device = apply_network_enrichment(device, enrichment);
            device = apply_network_lshw_enrichment(
                device,
                lshw.by_interface.get(&net.ifname),
                &lshw.source,
            );
            devices.push(device);
        }
        ProbeResult {
            devices,
            warnings,
            consumed: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct NetworkIpAddressInfo {
    ipv4: Vec<String>,
    ipv6: Vec<String>,
}

#[derive(Debug, Default)]
struct NetworkIpAddresses {
    source: String,
    by_interface: HashMap<String, NetworkIpAddressInfo>,
}

#[derive(Default)]
struct NetworkLshwRecords {
    source: String,
    by_interface: HashMap<String, LshwNetworkRecord>,
}

async fn network_ip_addresses(
    ctx: &ProbeContext<'_>,
    probe_name: &str,
    warnings: &mut Vec<ScanWarning>,
) -> NetworkIpAddresses {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("ip", ["-j", "addr"]), ctx.timeout)
        .await;
    if !result.is_success() {
        warnings.extend(ProbeResult::source_failure(probe_name, &result).warnings);
        return NetworkIpAddresses::default();
    }

    let records = match parse_ip_j_addr_result(&result.stdout) {
        Ok(records) => records,
        Err(err) => {
            warnings.push(
                ScanWarning::new(
                    "parse_failed",
                    format!(
                        "network source '{}' could not be parsed: {err}",
                        result.source
                    ),
                )
                .with_source(result.source),
            );
            return NetworkIpAddresses::default();
        }
    };

    let mut by_interface: HashMap<String, NetworkIpAddressInfo> = HashMap::new();
    for record in records {
        let entry = by_interface.entry(record.ifname).or_default();
        for addr in record.addr_info {
            let Some(local) = addr.local.filter(|local| !local.is_empty()) else {
                continue;
            };
            match addr.family.as_deref() {
                Some("inet") => entry.ipv4.push(local),
                Some("inet6") => entry.ipv6.push(local),
                _ => {}
            }
        }
    }

    NetworkIpAddresses {
        source: result.source,
        by_interface,
    }
}

async fn network_lshw_records(ctx: &ProbeContext<'_>) -> NetworkLshwRecords {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("lshw", ["-class", "network"]),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return NetworkLshwRecords::default();
    }

    let by_interface = parse_lshw_network(&result.stdout)
        .into_iter()
        .filter_map(|record| Some((record.logical_name.clone()?, record)))
        .collect();
    NetworkLshwRecords {
        source: result.source,
        by_interface,
    }
}

struct NetworkSysfsEnrichment {
    source: String,
    speed_mbps: Option<u32>,
    duplex: Option<String>,
    wireless: bool,
    ethernet: bool,
    driver: Option<String>,
    modules: Vec<String>,
    bus: Option<BusInfo>,
    contributed: bool,
}

impl NetworkSysfsEnrichment {
    fn network_type(&self) -> Option<String> {
        if self.wireless {
            Some("wireless".to_string())
        } else if self.ethernet {
            Some("ethernet".to_string())
        } else {
            None
        }
    }
}

async fn network_devices_from_sysfs(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let paths = ctx.runner.glob("/sys/class/net/*").await.paths;
    let lshw = network_lshw_records(ctx).await;
    let mut devices = Vec::new();

    for path in paths {
        let Some(ifname) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if is_ignored_network_interface(ifname) {
            continue;
        }

        let mac = read_optional_trimmed(ctx, &path.join("address")).await;
        let operstate = read_optional_trimmed(ctx, &path.join("operstate")).await;
        let enrichment = network_sysfs_enrichment(ctx, ifname).await;
        let device = Device::new(
            device_id::network(mac.as_deref(), ifname),
            DeviceKind::Network,
            ifname.to_string(),
            DeviceProperties::Network(NetworkInfo {
                interface: Some(ifname.to_string()),
                network_type: enrichment.network_type(),
                mac,
                operstate,
                speed_mbps: enrichment.speed_mbps,
                duplex: enrichment.duplex.clone(),
                ..Default::default()
            }),
        )
        .with_source(SourceEvidence {
            source: path.display().to_string(),
            kind: SourceKind::Sysfs,
            status: SourceStatus::Success,
            summary: None,
        });
        let device = apply_network_enrichment(device, enrichment);
        let device =
            apply_network_lshw_enrichment(device, lshw.by_interface.get(ifname), &lshw.source);
        devices.push(device);
    }

    devices
}

async fn network_sysfs_enrichment(ctx: &ProbeContext<'_>, ifname: &str) -> NetworkSysfsEnrichment {
    let path = Path::new("/sys/class/net").join(ifname);
    let speed_mbps = read_optional_trimmed(ctx, &path.join("speed"))
        .await
        .and_then(|speed| speed.parse().ok());
    let duplex = read_optional_trimmed(ctx, &path.join("duplex")).await;
    let wireless = !ctx
        .runner
        .glob(&format!("/sys/class/net/{ifname}/wireless"))
        .await
        .paths
        .is_empty();
    let uevent = read_optional_trimmed(ctx, &path.join("device/uevent")).await;
    let driver = uevent
        .as_deref()
        .and_then(|uevent| parse_uevent_value(uevent, "DRIVER"));
    let bus = uevent.as_deref().and_then(pci_bus_from_uevent);
    let modules = read_kernel_modules(ctx, &path.join("device")).await;
    let ethernet = !wireless
        && (driver.is_some() || speed_mbps.is_some() || duplex.is_some() || bus.is_some());

    NetworkSysfsEnrichment {
        source: path.display().to_string(),
        speed_mbps,
        duplex,
        wireless,
        ethernet,
        driver,
        modules,
        bus,
        contributed: false,
    }
}

fn apply_network_enrichment(mut device: Device, mut enrichment: NetworkSysfsEnrichment) -> Device {
    if let Some(bus) = enrichment.bus {
        device = device.with_bus(bus);
        enrichment.contributed = true;
    }
    if enrichment.wireless {
        device.capabilities.push("wireless".to_string());
        enrichment.contributed = true;
    } else if enrichment.ethernet {
        device.capabilities.push("ethernet".to_string());
        enrichment.contributed = true;
    }
    if enrichment.driver.is_some() || !enrichment.modules.is_empty() {
        device = device.with_driver(DriverInfo {
            name: enrichment.driver,
            version: None,
            modules: enrichment.modules,
            provider: None,
            status: DriverStatus::InUse,
        });
        enrichment.contributed = true;
    }
    if enrichment.speed_mbps.is_some() || enrichment.duplex.is_some() {
        enrichment.contributed = true;
    }
    if enrichment.contributed
        && !device
            .sources
            .iter()
            .any(|source| source.source == enrichment.source)
    {
        device = device.with_source(SourceEvidence {
            source: enrichment.source,
            kind: SourceKind::Sysfs,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

fn apply_network_lshw_enrichment(
    mut device: Device,
    record: Option<&LshwNetworkRecord>,
    source: &str,
) -> Device {
    let Some(record) = record else {
        return device;
    };
    let mut contributed = false;

    if device.vendor.is_none() && record.vendor.is_some() {
        device.vendor = record.vendor.clone();
        contributed = true;
    }
    if device.model.is_none() && record.product.is_some() {
        device.model = record.product.clone();
        contributed = true;
    }
    if device.bus.is_none() {
        if let Some(bus) = record.bus_info.as_deref().and_then(lshw_network_pci_bus) {
            device = device.with_bus(bus);
            contributed = true;
        }
    }
    if record.driver.is_some() || record.driver_version.is_some() {
        let mut driver = device.driver.take().unwrap_or(DriverInfo {
            name: None,
            version: None,
            modules: Vec::new(),
            provider: None,
            status: DriverStatus::InUse,
        });
        let original = driver.clone();
        driver.name = driver.name.or_else(|| record.driver.clone());
        driver.version = driver.version.or_else(|| record.driver_version.clone());
        contributed |= driver != original;
        device.driver = Some(driver);
    }
    if let DeviceProperties::Network(network) = &mut device.properties {
        if network.speed_mbps.is_none() && record.capacity_mbps.is_some() {
            network.speed_mbps = record.capacity_mbps;
            contributed = true;
        }
        if network.firmware.is_none() && record.firmware.is_some() {
            network.firmware = record.firmware.clone();
            contributed = true;
        }
    }
    if contributed
        && !source.is_empty()
        && !device.sources.iter().any(|entry| entry.source == source)
    {
        device = device.with_source(SourceEvidence {
            source: source.to_string(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

fn lshw_network_pci_bus(value: &str) -> Option<BusInfo> {
    Some(BusInfo::Pci {
        address: value.strip_prefix("pci@")?.to_string(),
        vendor_id: None,
        device_id: None,
        subsystem_vendor_id: None,
        subsystem_device_id: None,
        class: None,
    })
}

fn parse_uevent_value(input: &str, key: &str) -> Option<String> {
    input.lines().find_map(|line| {
        let (candidate, value) = line.split_once('=')?;
        (candidate == key && !value.is_empty()).then(|| value.to_string())
    })
}

async fn read_optional_trimmed(ctx: &ProbeContext<'_>, path: &Path) -> Option<String> {
    let result = ctx.runner.read_file(path).await;
    result
        .is_success()
        .then(|| result.stdout.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn storage_devices_from_sysfs(ctx: &ProbeContext<'_>) -> Vec<Device> {
    let mut devices = Vec::new();
    let lshw = storage_lshw_records(ctx).await;

    for path in ctx.runner.glob("/sys/block/*").await.paths {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if is_ignored_block_device(name) {
            continue;
        }

        let node = format!("/dev/{name}");
        let vendor = read_optional_trimmed(ctx, &path.join("device/vendor")).await;
        let model = read_optional_trimmed(ctx, &path.join("device/model")).await;
        let serial = read_optional_trimmed(ctx, &path.join("device/serial")).await;
        let wwn = read_first_optional_trimmed(ctx, &[path.join("device/wwid"), path.join("wwid")])
            .await
            .map(normalize_storage_wwn);
        let firmware = read_first_optional_trimmed(
            ctx,
            &[path.join("device/rev"), path.join("device/firmware_rev")],
        )
        .await;
        let size_bytes = read_optional_trimmed(ctx, &path.join("size"))
            .await
            .and_then(|sectors| sectors.parse::<u64>().ok())
            .and_then(|sectors| sectors.checked_mul(512));
        let media_type = read_optional_trimmed(ctx, &path.join("queue/rotational"))
            .await
            .and_then(|rotational| match rotational.as_str() {
                "0" => Some("ssd".to_string()),
                "1" => Some("hdd".to_string()),
                _ => None,
            });

        let mut device = Device::new(
            device_id::storage(None, serial.as_deref(), &node),
            DeviceKind::Storage,
            model.clone().unwrap_or_else(|| node.clone()),
            DeviceProperties::Storage(StorageInfo {
                device_node: Some(node),
                size_bytes,
                media_type,
                firmware,
                wwn,
                ..Default::default()
            }),
        )
        .with_source(SourceEvidence {
            source: path.display().to_string(),
            kind: SourceKind::Sysfs,
            status: SourceStatus::Success,
            summary: None,
        });
        device.vendor = vendor;
        device.model = model;
        device.serial = serial;
        let device = apply_storage_driver(ctx, device, name).await;
        let device = apply_storage_lshw_enrichment(device, &lshw);
        devices.push(apply_storage_smartctl(ctx, device).await);
    }

    devices
}

#[derive(Default)]
struct StorageLshwRecords {
    source: String,
    by_node: HashMap<String, LshwDiskRecord>,
}

async fn storage_lshw_records(ctx: &ProbeContext<'_>) -> StorageLshwRecords {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("lshw", ["-class", "disk"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return StorageLshwRecords::default();
    }

    let by_node = parse_lshw_disk(&result.stdout)
        .into_iter()
        .filter_map(|record| Some((record.logical_name.clone()?, record)))
        .collect();
    StorageLshwRecords {
        source: result.source,
        by_node,
    }
}

async fn apply_storage_driver(ctx: &ProbeContext<'_>, device: Device, name: &str) -> Device {
    let sysfs_path = Path::new("/sys/block").join(name);
    let Some(driver) = read_optional_trimmed(ctx, &sysfs_path.join("device/uevent"))
        .await
        .and_then(|uevent| parse_uevent_value(&uevent, "DRIVER"))
    else {
        return device;
    };

    let mut device = device.with_driver(DriverInfo {
        name: Some(driver),
        version: None,
        modules: Vec::new(),
        provider: None,
        status: DriverStatus::InUse,
    });
    let source = sysfs_path.display().to_string();
    if !device
        .sources
        .iter()
        .any(|evidence| evidence.source == source)
    {
        device = device.with_source(SourceEvidence {
            source,
            kind: SourceKind::Sysfs,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

fn apply_storage_lshw_enrichment(mut device: Device, lshw: &StorageLshwRecords) -> Device {
    let Some(node) = (match &device.properties {
        DeviceProperties::Storage(storage) => storage.device_node.clone(),
        _ => None,
    }) else {
        return device;
    };
    let Some(record) = lshw.by_node.get(&node) else {
        return device;
    };
    let mut contributed = false;

    if device.vendor.is_none() && record.vendor.is_some() {
        device.vendor = record.vendor.clone();
        contributed = true;
    }
    if device.model.is_none() && record.product.is_some() {
        device.model = record.product.clone();
        if device.name == node {
            device.name = record.product.clone().unwrap_or(device.name);
        }
        contributed = true;
    }
    if device.serial.is_none() && record.serial.is_some() {
        device.serial = record.serial.clone();
        contributed = true;
    }
    if let DeviceProperties::Storage(storage) = &mut device.properties {
        if storage.firmware.is_none() && record.firmware.is_some() {
            storage.firmware = record.firmware.clone();
            contributed = true;
        }
    }
    if contributed
        && !lshw.source.is_empty()
        && !device
            .sources
            .iter()
            .any(|source| source.source == lshw.source)
    {
        device = device.with_source(SourceEvidence {
            source: lshw.source.clone(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

async fn apply_storage_smartctl(ctx: &ProbeContext<'_>, mut device: Device) -> Device {
    let Some(node) = (match &device.properties {
        DeviceProperties::Storage(storage) => storage.device_node.clone(),
        _ => None,
    }) else {
        return device;
    };

    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("smartctl", ["-a".to_string(), "-j".to_string(), node]),
            ctx.timeout,
        )
        .await;
    let source_status = if result.is_success() {
        SourceStatus::Success
    } else {
        SourceStatus::Failed
    };
    if !result.is_success() && result.stdout.trim().is_empty() {
        return device;
    }

    let Ok(smart) = parse_smartctl_json(&result.stdout) else {
        return device;
    };
    if smart.smart_status.is_none() && smart.temperature_celsius.is_none() {
        return device;
    }
    if let DeviceProperties::Storage(storage) = &mut device.properties {
        storage.smart_status = storage.smart_status.take().or(smart.smart_status);
        storage.temperature_celsius = storage.temperature_celsius.or(smart.temperature_celsius);
    }

    device.with_source(SourceEvidence {
        source: result.source,
        kind: SourceKind::Command,
        status: source_status,
        summary: None,
    })
}

fn is_ignored_block_device(name: &str) -> bool {
    ["loop", "ram", "zram", "dm-", "md", "sr"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

async fn read_first_optional_trimmed(
    ctx: &ProbeContext<'_>,
    paths: &[std::path::PathBuf],
) -> Option<String> {
    for path in paths {
        if let Some(value) = read_optional_trimmed(ctx, path).await {
            return Some(value);
        }
    }
    None
}

fn normalize_storage_wwn(value: String) -> String {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(&value)
        .to_string()
}

#[async_trait]
impl Probe for StorageProbe {
    fn name(&self) -> &'static str {
        "storage"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Storage]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx
            .runner
            .run_command(
                &CommandSpec::new(
                    "lsblk",
                    ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
                ),
                ctx.timeout,
            )
            .await;
        if !result.is_success() {
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            fallback.devices = storage_devices_from_sysfs(ctx).await;
            return fallback;
        }
        let records = match parse_lsblk_json_result(&result.stdout) {
            Ok(records) => records,
            Err(err) => {
                return ProbeResult {
                    devices: Vec::new(),
                    warnings: vec![ScanWarning::new(
                        "parse_failed",
                        format!(
                            "storage source '{}' could not be parsed: {err}",
                            result.source
                        ),
                    )
                    .with_source(result.source)],
                    consumed: Vec::new(),
                };
            }
        };
        let lshw = storage_lshw_records(ctx).await;
        let mut devices = Vec::new();
        for dev in records {
            if dev.device_type.as_deref() != Some("disk") {
                continue;
            }
            let name = dev.name;
            let node = format!("/dev/{name}");
            let wwn = dev.wwn.map(normalize_storage_wwn);
            let mut device = Device::new(
                device_id::storage(wwn.as_deref(), dev.serial.as_deref(), &node),
                DeviceKind::Storage,
                dev.model.clone().unwrap_or_else(|| node.clone()),
                DeviceProperties::Storage(StorageInfo {
                    device_node: Some(node),
                    size_bytes: dev.size,
                    media_type: dev.tran,
                    firmware: dev.rev,
                    wwn,
                    ..Default::default()
                }),
            )
            .with_source(SourceEvidence {
                source: result.source.clone(),
                kind: SourceKind::Command,
                status: SourceStatus::Success,
                summary: None,
            });
            device.model = dev.model;
            device.serial = dev.serial;
            let device = apply_storage_driver(ctx, device, &name).await;
            let device = apply_storage_lshw_enrichment(device, &lshw);
            devices.push(apply_storage_smartctl(ctx, device).await);
        }
        ProbeResult::with_devices(devices)
    }
}

#[async_trait]
impl Probe for MemoryProbe {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Memory]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx
            .runner
            .run_command(
                &CommandSpec::new("dmidecode", ["-t", "memory"]),
                ctx.timeout,
            )
            .await;
        if !result.is_success() {
            return memory_fallback_from_lshw_or_proc(
                ctx,
                ProbeResult::source_failure(self.name(), &result),
            )
            .await;
        }
        let records = parse_dmidecode_memory(&result.stdout);
        if records.is_empty() {
            let fallback = ProbeResult {
                devices: Vec::new(),
                warnings: vec![ScanWarning::new(
                    "source_empty",
                    "memory source produced no DIMM records",
                )
                .with_source(result.source)],
                consumed: Vec::new(),
            };
            return memory_fallback_from_lshw_or_proc(ctx, fallback).await;
        }
        let devices = memory_devices_from_records(records, &result.source, SourceKind::Command);
        ProbeResult::with_devices(devices)
    }
}

async fn memory_fallback_from_lshw_or_proc(
    ctx: &ProbeContext<'_>,
    mut fallback: ProbeResult,
) -> ProbeResult {
    let lshw_result = ctx
        .runner
        .run_command(&CommandSpec::new("lshw", ["-class", "memory"]), ctx.timeout)
        .await;
    if lshw_result.is_success() {
        let records = parse_lshw_memory(&lshw_result.stdout);
        if !records.is_empty() {
            fallback.devices =
                memory_devices_from_records(records, &lshw_result.source, SourceKind::Command);
            return fallback;
        }
        fallback.warnings.push(
            ScanWarning::new(
                "source_empty",
                "lshw memory source produced no DIMM records",
            )
            .with_source(lshw_result.source),
        );
    } else {
        fallback
            .warnings
            .extend(ProbeResult::source_failure("memory", &lshw_result).warnings);
    }

    let proc_meminfo_result = ctx.runner.read_file(Path::new("/proc/meminfo")).await;
    if proc_meminfo_result.is_success() {
        if let Some(size_bytes) = parse_proc_meminfo_total_bytes(&proc_meminfo_result.stdout) {
            let device = Device::new(
                "memory:system",
                DeviceKind::Memory,
                "System Memory",
                DeviceProperties::Memory(MemoryInfo {
                    size_bytes: Some(size_bytes),
                    ..Default::default()
                }),
            )
            .with_source(SourceEvidence {
                source: proc_meminfo_result.source,
                kind: SourceKind::Procfs,
                status: SourceStatus::Success,
                summary: None,
            });
            fallback.devices.push(device);
        }
    }
    fallback
}

fn memory_devices_from_records(
    records: Vec<DmiMemoryRecord>,
    source: &str,
    source_kind: SourceKind,
) -> Vec<Device> {
    records
        .into_iter()
        .enumerate()
        .map(|(idx, mem)| {
            let id = mem
                .serial
                .as_ref()
                .filter(|serial| !serial.trim().is_empty())
                .map(|serial| format!("memory:serial:{serial}"))
                .unwrap_or_else(|| {
                    format!(
                        "memory:slot:{}",
                        mem.locator.clone().unwrap_or_else(|| idx.to_string())
                    )
                });
            Device::new(
                id,
                DeviceKind::Memory,
                mem.locator
                    .clone()
                    .unwrap_or_else(|| format!("Memory DIMM {idx}")),
                DeviceProperties::Memory(MemoryInfo {
                    size_bytes: parse_size_to_bytes(mem.size.as_deref()),
                    vendor: mem.manufacturer,
                    memory_type: mem.memory_type,
                    speed_mtps: parse_speed_mtps(mem.speed.as_deref()),
                    locator: mem.locator,
                    serial: mem.serial,
                    part_number: mem.part_number,
                }),
            )
            .with_source(SourceEvidence {
                source: source.to_string(),
                kind: source_kind,
                status: SourceStatus::Success,
                summary: None,
            })
        })
        .collect()
}

#[async_trait]
impl Probe for BiosProbe {
    fn name(&self) -> &'static str {
        "bios"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Bios, DeviceKind::Motherboard]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx
            .runner
            .run_command(
                &CommandSpec::new("dmidecode", ["-t", "0,1,2,3"]),
                ctx.timeout,
            )
            .await;
        if !result.is_success() {
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            if let Some(dmi) = read_sysfs_dmi(ctx).await {
                fallback.devices = bios_board_devices(dmi, "/sys/class/dmi/id", SourceKind::Sysfs);
            }
            return fallback;
        }
        let dmi = parse_dmidecode_bios_board(&result.stdout);
        if dmi == Default::default() {
            return ProbeResult {
                devices: Vec::new(),
                warnings: vec![ScanWarning::new(
                    "source_empty",
                    "bios source produced no DMI records",
                )
                .with_source(result.source)],
                consumed: Vec::new(),
            };
        }
        ProbeResult::with_devices(bios_board_devices(dmi, &result.source, SourceKind::Command))
    }
}

async fn read_sysfs_dmi(ctx: &ProbeContext<'_>) -> Option<DmiBiosBoardRecord> {
    let dmi = DmiBiosBoardRecord {
        bios_vendor: read_sysfs_dmi_value(ctx, "bios_vendor").await,
        bios_version: read_sysfs_dmi_value(ctx, "bios_version").await,
        bios_release_date: read_sysfs_dmi_value(ctx, "bios_date").await,
        board_manufacturer: read_sysfs_dmi_value(ctx, "board_vendor").await,
        board_product_name: read_sysfs_dmi_value(ctx, "board_name").await,
        board_serial: read_sysfs_dmi_value(ctx, "board_serial").await,
    };

    if dmi == Default::default() {
        None
    } else {
        Some(dmi)
    }
}

async fn read_sysfs_dmi_value(ctx: &ProbeContext<'_>, name: &str) -> Option<String> {
    let path = Path::new("/sys/class/dmi/id").join(name);
    let result = ctx.runner.read_file(&path).await;
    if result.is_success() {
        clean_sysfs_dmi_value(&result.stdout)
    } else {
        None
    }
}

fn clean_sysfs_dmi_value(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && !value.eq_ignore_ascii_case("Not Specified")).then(|| value.to_string())
}

fn bios_board_devices(
    dmi: DmiBiosBoardRecord,
    source: &str,
    source_kind: SourceKind,
) -> Vec<Device> {
    let bios = Device::new(
        "bios:0",
        DeviceKind::Bios,
        dmi.bios_version
            .clone()
            .unwrap_or_else(|| "BIOS".to_string()),
        DeviceProperties::Bios(BiosInfo {
            vendor: dmi.bios_vendor,
            version: dmi.bios_version,
            release_date: dmi.bios_release_date,
            ..Default::default()
        }),
    )
    .with_source(SourceEvidence {
        source: source.to_string(),
        kind: source_kind,
        status: SourceStatus::Success,
        summary: None,
    });
    let board = Device::new(
        dmi.board_serial
            .as_ref()
            .map(|serial| format!("motherboard:serial:{serial}"))
            .unwrap_or_else(|| "motherboard:0".to_string()),
        DeviceKind::Motherboard,
        dmi.board_product_name
            .clone()
            .unwrap_or_else(|| "Motherboard".to_string()),
        DeviceProperties::Motherboard(MotherboardInfo {
            manufacturer: dmi.board_manufacturer,
            product_name: dmi.board_product_name,
            serial: dmi.board_serial,
            ..Default::default()
        }),
    )
    .with_source(SourceEvidence {
        source: source.to_string(),
        kind: source_kind,
        status: SourceStatus::Success,
        summary: None,
    });
    vec![bios, board]
}

#[async_trait]
impl Probe for GpuProbe {
    fn name(&self) -> &'static str {
        "gpu"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Gpu]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let lshw = gpu_lshw_display_records(ctx).await;
        let result = ctx
            .runner
            .run_command(&CommandSpec::new("lspci", ["-nn", "-k"]), ctx.timeout)
            .await;
        if !result.is_success() {
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            fallback.devices = gpu_devices_from_sysfs_pci(ctx, &mut fallback.consumed, &lshw).await;
            return fallback;
        }
        let mut probe_result = ProbeResult::default();
        for gpu in parse_gpu_lspci(&result.stdout) {
            let pci_id = device_id::pci(&gpu.address);
            let vendor = gpu
                .vendor
                .as_deref()
                .and_then(normalize_gpu_vendor)
                .map(str::to_string)
                .or_else(|| {
                    gpu.device
                        .as_deref()
                        .and_then(normalize_gpu_vendor)
                        .map(str::to_string)
                })
                .or_else(|| {
                    gpu.vendor_id
                        .as_deref()
                        .and_then(normalize_gpu_vendor_id)
                        .map(str::to_string)
                })
                .or_else(|| gpu.vendor.clone())
                .or_else(|| gpu.device.clone());
            probe_result.consumed.push(DeviceRef { id: pci_id });
            let device = Device::new(
                device_id::other("gpu:pci", &gpu.address),
                DeviceKind::Gpu,
                gpu.device
                    .clone()
                    .or(gpu.vendor.clone())
                    .unwrap_or_else(|| "GPU".to_string()),
                DeviceProperties::Gpu(GpuInfo {
                    vendor,
                    ..Default::default()
                }),
            )
            .with_bus(BusInfo::Pci {
                address: gpu.address,
                vendor_id: gpu.vendor_id,
                device_id: gpu.device_id,
                subsystem_vendor_id: gpu.subsystem_vendor_id,
                subsystem_device_id: gpu.subsystem_device_id,
                class: gpu.class_id,
            })
            .with_driver(DriverInfo {
                name: gpu.kernel_driver,
                version: None,
                modules: gpu.kernel_modules,
                provider: None,
                status: DriverStatus::InUse,
            })
            .with_source(SourceEvidence {
                source: result.source.clone(),
                kind: SourceKind::Command,
                status: SourceStatus::Success,
                summary: None,
            });
            probe_result
                .devices
                .push(apply_gpu_lshw_enrichment(device, &lshw));
        }
        probe_result
    }
}

#[derive(Default)]
struct GpuLshwDisplayRecords {
    source: String,
    by_pci_address: HashMap<String, LshwDisplayRecord>,
}

async fn gpu_lshw_display_records(ctx: &ProbeContext<'_>) -> GpuLshwDisplayRecords {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("lshw", ["-class", "display"]),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return GpuLshwDisplayRecords::default();
    }

    let by_pci_address = parse_lshw_display(&result.stdout)
        .into_iter()
        .filter_map(|record| {
            Some((
                lshw_display_pci_address(record.bus_info.as_deref()?)?,
                record,
            ))
        })
        .collect();
    GpuLshwDisplayRecords {
        source: result.source,
        by_pci_address,
    }
}

fn apply_gpu_lshw_enrichment(mut device: Device, lshw: &GpuLshwDisplayRecords) -> Device {
    let Some(address) = device.bus.as_ref().and_then(gpu_pci_address) else {
        return device;
    };
    let Some(record) = lshw.by_pci_address.get(address) else {
        return device;
    };
    let mut contributed = false;

    if let Some(product) = record.product.clone() {
        if device.model.is_none() {
            device.model = Some(product.clone());
            contributed = true;
        }
        if is_generic_gpu_label(&device.name) {
            device.name = product;
            contributed = true;
        }
    }
    if let Some(vendor) = record
        .vendor
        .as_deref()
        .and_then(normalize_gpu_vendor)
        .map(str::to_string)
        .or_else(|| record.vendor.clone())
    {
        if device.vendor.is_none() {
            device.vendor = Some(vendor.clone());
            contributed = true;
        }
        if let DeviceProperties::Gpu(gpu) = &mut device.properties {
            if gpu.vendor.as_deref().is_none_or(is_generic_gpu_label) {
                gpu.vendor = Some(vendor);
                contributed = true;
            }
        }
    }
    if record.driver.is_some() {
        let mut driver = device.driver.take().unwrap_or(DriverInfo {
            name: None,
            version: None,
            modules: Vec::new(),
            provider: None,
            status: DriverStatus::InUse,
        });
        let original = driver.clone();
        driver.name = driver.name.or_else(|| record.driver.clone());
        contributed |= driver != original;
        device.driver = Some(driver);
    }
    if contributed
        && !lshw.source.is_empty()
        && !device
            .sources
            .iter()
            .any(|source| source.source == lshw.source)
    {
        device = device.with_source(SourceEvidence {
            source: lshw.source.clone(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

fn gpu_pci_address(bus: &BusInfo) -> Option<&str> {
    match bus {
        BusInfo::Pci { address, .. } => Some(address),
        _ => None,
    }
}

fn lshw_display_pci_address(value: &str) -> Option<String> {
    value.strip_prefix("pci@").map(ToString::to_string)
}

fn is_generic_gpu_label(value: &str) -> bool {
    let value = value.trim();
    value.eq_ignore_ascii_case("gpu")
        || value.eq_ignore_ascii_case("device")
        || value.to_ascii_lowercase().starts_with("gpu ")
}

async fn gpu_devices_from_sysfs_pci(
    ctx: &ProbeContext<'_>,
    consumed: &mut Vec<DeviceRef>,
    lshw: &GpuLshwDisplayRecords,
) -> Vec<Device> {
    let mut devices = Vec::new();
    for record in read_sysfs_pci_records(ctx).await {
        if !record
            .class_id
            .as_deref()
            .is_some_and(|class| class.starts_with("03"))
        {
            continue;
        }

        let vendor = record
            .vendor_id
            .as_deref()
            .and_then(normalize_gpu_vendor_id)
            .map(str::to_string);

        consumed.push(DeviceRef {
            id: device_id::pci(&record.address),
        });
        let driver = record.driver.clone();
        let modules = record.modules.clone();
        let mut device = Device::new(
            device_id::other("gpu:pci", &record.address),
            DeviceKind::Gpu,
            vendor
                .clone()
                .unwrap_or_else(|| format!("GPU {}", record.address)),
            DeviceProperties::Gpu(GpuInfo {
                vendor,
                ..Default::default()
            }),
        )
        .with_bus(BusInfo::Pci {
            address: record.address,
            vendor_id: record.vendor_id,
            device_id: record.device_id,
            subsystem_vendor_id: record.subsystem_vendor_id,
            subsystem_device_id: record.subsystem_device_id,
            class: record.class_id,
        })
        .with_source(SourceEvidence {
            source: record.path.display().to_string(),
            kind: SourceKind::Sysfs,
            status: SourceStatus::Success,
            summary: None,
        });
        if driver.is_some() {
            device = device.with_driver(DriverInfo {
                name: driver,
                version: None,
                modules,
                provider: None,
                status: DriverStatus::InUse,
            });
        }
        devices.push(apply_gpu_lshw_enrichment(device, lshw));
    }

    devices
}

#[async_trait]
impl Probe for MonitorProbe {
    fn name(&self) -> &'static str {
        "monitor"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Monitor]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let result = ctx
            .runner
            .run_command(&CommandSpec::new("xrandr", ["--query"]), ctx.timeout)
            .await;
        let verbose_result = ctx
            .runner
            .run_command(&CommandSpec::new("xrandr", ["--verbose"]), ctx.timeout)
            .await;

        let mut warnings = Vec::new();
        let mut edids: HashMap<String, Vec<(Vec<u8>, String)>> = HashMap::new();
        let mut sysfs_paths: HashMap<String, Vec<_>> = HashMap::new();
        for path in ctx.runner.glob("/sys/class/drm/*/edid").await.paths {
            let Some(connector) = normalize_sysfs_connector(&path) else {
                continue;
            };
            sysfs_paths.entry(connector).or_default().push(path);
        }
        for (connector, paths) in sysfs_paths {
            if paths.len() != 1 {
                continue;
            }
            let path = &paths[0];
            let bytes_result = ctx.runner.read_file_bytes(path).await;
            if bytes_result.is_success() {
                edids
                    .entry(connector)
                    .or_default()
                    .push((bytes_result.bytes, bytes_result.source));
            } else {
                warnings.push(source_bytes_failure(self.name(), &bytes_result));
            }
        }
        if verbose_result.is_success() {
            for record in parse_xrandr_verbose(&verbose_result.stdout) {
                edids
                    .entry(record.connector)
                    .or_default()
                    .insert(0, (record.edid, verbose_result.source.clone()));
            }
        }

        let mut monitors: Vec<_> = if result.is_success() {
            parse_xrandr_query(&result.stdout)
                .into_iter()
                .filter(|mon| mon.connected)
                .map(|mon| {
                    (
                        mon.connector,
                        mon.resolution,
                        result.source.clone(),
                        SourceKind::Command,
                        false,
                    )
                })
                .collect()
        } else {
            warnings.extend(ProbeResult::source_failure(self.name(), &result).warnings);
            edids
                .keys()
                .map(|connector| {
                    (
                        connector.clone(),
                        None,
                        result.source.clone(),
                        SourceKind::Command,
                        true,
                    )
                })
                .collect()
        };
        monitors.sort_by(|left, right| left.0.cmp(&right.0));

        let devices = monitors
            .into_iter()
            .filter_map(
                |(connector, resolution, mut source, mut source_kind, require_edid)| {
                    let id = device_id::other("monitor", &connector);
                    let mut info = MonitorInfo {
                        connector: Some(connector.clone()),
                        resolution,
                        ..Default::default()
                    };
                    let valid_edid_source = edids.get(&connector).and_then(|candidates| {
                        apply_first_edid(&mut info, &id, candidates, &mut warnings)
                    });
                    if require_edid {
                        let valid_edid_source = valid_edid_source?;
                        source_kind = source_kind_for_monitor_edid_source(&valid_edid_source);
                        source = valid_edid_source;
                    }

                    Some(
                        Device::new(
                            id,
                            DeviceKind::Monitor,
                            connector,
                            DeviceProperties::Monitor(info),
                        )
                        .with_source(SourceEvidence {
                            source,
                            kind: source_kind,
                            status: SourceStatus::Success,
                            summary: None,
                        }),
                    )
                },
            )
            .collect();
        ProbeResult {
            devices,
            warnings,
            consumed: Vec::new(),
        }
    }
}

fn is_ignored_network_interface(ifname: &str) -> bool {
    if ifname == "lo" {
        return true;
    }
    let prefixes = [
        "docker", "veth", "br-", "virbr", "lxcbr", "cni", "flannel", "tun", "tap",
    ];
    prefixes.iter().any(|prefix| ifname.starts_with(prefix))
}

fn apply_first_edid(
    info: &mut MonitorInfo,
    id: &str,
    candidates: &[(Vec<u8>, String)],
    warnings: &mut Vec<ScanWarning>,
) -> Option<String> {
    for (bytes, source) in candidates {
        match parse_edid(bytes) {
            Ok(edid) => {
                apply_edid(info, edid);
                return Some(source.clone());
            }
            Err(err) => warnings.push(
                ScanWarning::new(
                    "edid_parse_failed",
                    format!("monitor EDID source '{source}' failed: {err:?}"),
                )
                .with_source(source.clone())
                .with_device_id(id.to_string()),
            ),
        }
    }
    None
}

fn source_bytes_failure(probe: &str, result: &SourceBytesResult) -> ScanWarning {
    let kind = result.error_kind.unwrap_or(SourceErrorKind::Failed);
    let code = match kind {
        SourceErrorKind::Missing => "source_missing",
        SourceErrorKind::PermissionDenied => "source_permission_denied",
        SourceErrorKind::Timeout => "source_timeout",
        SourceErrorKind::Failed => "source_failed",
    };
    let detail = result.stderr.trim();
    let message = if detail.is_empty() {
        format!("{probe} source '{}' failed: {kind:?}", result.source)
    } else {
        format!("{probe} source '{}' failed: {detail}", result.source)
    };
    ScanWarning::new(code, message).with_source(result.source.clone())
}

fn source_kind_for_monitor_edid_source(source: &str) -> SourceKind {
    if source.starts_with("/sys/") {
        SourceKind::Sysfs
    } else {
        SourceKind::Command
    }
}

fn apply_edid(info: &mut MonitorInfo, edid: hw_parser::EdidRecord) {
    info.manufacturer = edid.manufacturer.clone();
    info.manufacturer_name = edid
        .manufacturer
        .as_deref()
        .and_then(lookup_pnp_manufacturer)
        .map(str::to_string);
    info.product = edid.name;
    info.product_code = edid.product_code;
    info.serial = edid.serial;
    info.manufactured_year = edid.year;
    info.manufactured_week = edid.week;
    info.size_cm = edid.size_cm;
    info.preferred_width = edid.preferred_mode.as_ref().map(|mode| mode.width);
    info.preferred_height = edid.preferred_mode.as_ref().map(|mode| mode.height);
    info.preferred_refresh_hz = edid.preferred_mode.as_ref().map(|mode| mode.refresh_hz);
}

fn normalize_sysfs_connector(path: &Path) -> Option<String> {
    let name = path.parent()?.file_name()?.to_str()?;
    let connector = name
        .strip_prefix("card")
        .and_then(|rest| rest.split_once('-').map(|(_, connector)| connector))
        .unwrap_or(name);
    Some(connector.replace("HDMI-A-", "HDMI-"))
}
