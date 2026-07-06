use crate::{
    sysfs_pci::{is_pci_address, pci_bus_from_uevent, read_kernel_modules, read_sysfs_pci_records},
    Probe, ProbeContext, ProbeResult,
};
use async_trait::async_trait;
use hw_model::{
    device_id, BiosInfo, BusInfo, CpuInfo, Device, DeviceKind, DeviceProperties, DeviceRef,
    DriverInfo, DriverStatus, GpuInfo, MemoryInfo, MonitorInfo, MotherboardInfo, NetworkInfo,
    ScanWarning, SourceEvidence, SourceKind, SourceStatus, StorageInfo, SystemDeviceInfo,
};
use hw_parser::{
    infer_cpu_vendor_from_name, lookup_pnp_manufacturer, merge_cpu_records, normalize_arch,
    normalize_cpu_vendor_id, normalize_gpu_vendor, normalize_gpu_vendor_id, parse_dmesg_gpu_vram,
    parse_dmidecode_bios_board, parse_dmidecode_memory, parse_dmidecode_processor,
    parse_dmidecode_system, parse_edid, parse_glxinfo_basic, parse_gpu_lspci,
    parse_hdparm_identify, parse_hwinfo_disk, parse_hwinfo_monitor, parse_ip_j_addr_result,
    parse_ip_j_link_result, parse_lsblk_json_result, parse_lscpu, parse_lshw_disk,
    parse_lshw_display, parse_lshw_memory, parse_lshw_network, parse_lshw_processor,
    parse_lshw_storage, parse_lspci_nn_k, parse_nvidia_settings_videoram,
    parse_nvidia_smi_memory_csv, parse_proc_cpuinfo, parse_proc_hardware,
    parse_proc_meminfo_total_bytes, parse_size_to_bytes, parse_smartctl_json,
    parse_spd_decode_dimms, parse_spd_eeprom, parse_speed_mtps, parse_voltage_v, parse_width_bits,
    parse_xrandr_query, parse_xrandr_verbose, DmesgGpuVramRecord, DmiBiosBoardRecord,
    DmiMemoryRecord, DmiSystemRecord, GlxinfoBasicRecord, HwinfoDiskRecord, HwinfoMonitorRecord,
    LshwDiskRecord, LshwDisplayRecord, LshwNetworkRecord, LshwStorageRecord, PciRecord,
};
use hw_source::{CommandSpec, SourceBytesResult, SourceErrorKind};
use std::{collections::HashMap, path::Path};

pub struct CpuProbe;
pub struct SystemProbe;
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
        let cpufreq = read_cpu_cpufreq(ctx).await;

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
        let cpufreq_contributed = cpufreq.is_some();
        if lscpu.is_none()
            && lshw.is_none()
            && dmi.is_empty()
            && proc_cpuinfo.is_none()
            && proc_hardware.is_none()
            && cpufreq.is_none()
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
        let mut merged = merge_cpu_records(
            merge_cpu_record_fallback(
                merge_cpu_record_fallback(lscpu, proc_cpuinfo),
                proc_hardware,
            ),
            lshw,
            &dmi,
        );
        if let Some(cpufreq) = cpufreq {
            merged.max_freq_mhz = merged.max_freq_mhz.or(cpufreq.cpu_max_mhz);
            merged.min_freq_mhz = merged.min_freq_mhz.or(cpufreq.cpu_min_mhz);
            merged.current_freq_mhz = merged.current_freq_mhz.or(cpufreq.cpu_mhz);
        }
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
                l1d_cache: merged.l1d_cache,
                l1i_cache: merged.l1i_cache,
                l2_cache: merged.l2_cache,
                l3_cache: merged.l3_cache,
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
        if cpufreq_contributed {
            device = device.with_source(SourceEvidence {
                source: "/sys/devices/system/cpu/cpu0/cpufreq".to_string(),
                kind: SourceKind::Sysfs,
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

#[async_trait]
impl Probe for SystemProbe {
    fn name(&self) -> &'static str {
        "system"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::System]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let runtime = read_system_runtime_info(ctx).await;
        let dmi_result = ctx
            .runner
            .run_command(&CommandSpec::new("dmidecode", ["-t", "1"]), ctx.timeout)
            .await;
        let mut warnings = Vec::new();
        let mut dmi_source = None;
        let dmi = if dmi_result.is_success() {
            let record = parse_dmidecode_system(&dmi_result.stdout);
            if record.is_empty() {
                warnings.push(
                    ScanWarning::new(
                        "source_empty",
                        "system source produced no DMI System Information fields",
                    )
                    .with_source(dmi_result.source),
                );
                let record = read_sysfs_system_dmi(ctx).await;
                if record.is_some() {
                    dmi_source = Some(SourceEvidence {
                        source: "/sys/class/dmi/id".to_string(),
                        kind: SourceKind::Sysfs,
                        status: SourceStatus::Success,
                        summary: None,
                    });
                }
                record
            } else {
                dmi_source = Some(SourceEvidence {
                    source: dmi_result.source,
                    kind: SourceKind::Command,
                    status: SourceStatus::Success,
                    summary: None,
                });
                Some(record)
            }
        } else {
            warnings.extend(ProbeResult::source_failure(self.name(), &dmi_result).warnings);
            let record = read_sysfs_system_dmi(ctx).await;
            if record.is_some() {
                dmi_source = Some(SourceEvidence {
                    source: "/sys/class/dmi/id".to_string(),
                    kind: SourceKind::Sysfs,
                    status: SourceStatus::Success,
                    summary: None,
                });
            }
            record
        };

        let Some(device) = system_device(runtime, dmi, dmi_source) else {
            return ProbeResult {
                devices: Vec::new(),
                warnings,
                consumed: Vec::new(),
            };
        };

        ProbeResult {
            devices: vec![device],
            warnings,
            consumed: Vec::new(),
        }
    }
}

#[derive(Default)]
struct SystemRuntimeInfo {
    hostname: Option<String>,
    os: Option<String>,
    kernel: Option<String>,
    architecture: Option<String>,
    sources: Vec<SourceEvidence>,
}

async fn read_system_runtime_info(ctx: &ProbeContext<'_>) -> SystemRuntimeInfo {
    let mut info = SystemRuntimeInfo::default();

    let hostname_result = ctx
        .runner
        .read_file(Path::new("/proc/sys/kernel/hostname"))
        .await;
    if hostname_result.is_success() {
        info.hostname = clean_sysfs_dmi_value(&hostname_result.stdout);
        if info.hostname.is_some() {
            info.sources.push(SourceEvidence {
                source: hostname_result.source,
                kind: SourceKind::Procfs,
                status: SourceStatus::Success,
                summary: None,
            });
        }
    }

    let os_release_result = ctx.runner.read_file(Path::new("/etc/os-release")).await;
    if os_release_result.is_success() {
        info.os = parse_os_pretty_name(&os_release_result.stdout);
        if info.os.is_some() {
            info.sources.push(SourceEvidence {
                source: os_release_result.source,
                kind: SourceKind::File,
                status: SourceStatus::Success,
                summary: None,
            });
        }
    }

    let kernel_result = ctx
        .runner
        .run_command(&CommandSpec::new("uname", ["-r"]), ctx.timeout)
        .await;
    if kernel_result.is_success() {
        info.kernel = clean_sysfs_dmi_value(&kernel_result.stdout);
        if info.kernel.is_some() {
            info.sources.push(SourceEvidence {
                source: kernel_result.source,
                kind: SourceKind::Command,
                status: SourceStatus::Success,
                summary: None,
            });
        }
    }

    let arch_result = ctx
        .runner
        .run_command(&CommandSpec::new("uname", ["-m"]), ctx.timeout)
        .await;
    if arch_result.is_success() {
        info.architecture = clean_sysfs_dmi_value(&arch_result.stdout);
        if info.architecture.is_some() {
            info.sources.push(SourceEvidence {
                source: arch_result.source,
                kind: SourceKind::Command,
                status: SourceStatus::Success,
                summary: None,
            });
        }
    }

    info
}

fn parse_os_pretty_name(input: &str) -> Option<String> {
    input.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key == "PRETTY_NAME").then(|| {
            value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string()
        })
    })
}

async fn read_sysfs_system_dmi(ctx: &ProbeContext<'_>) -> Option<DmiSystemRecord> {
    let record = DmiSystemRecord {
        manufacturer: read_sysfs_dmi_value(ctx, "sys_vendor").await,
        product_name: read_sysfs_dmi_value(ctx, "product_name").await,
        version: read_sysfs_dmi_value(ctx, "product_version").await,
        serial: read_sysfs_dmi_value(ctx, "product_serial").await,
        uuid: read_sysfs_dmi_value(ctx, "product_uuid").await,
        wake_up_type: None,
        sku_number: read_sysfs_dmi_value(ctx, "product_sku").await,
        family: read_sysfs_dmi_value(ctx, "product_family").await,
    };
    (!record.is_empty()).then_some(record)
}

fn system_device(
    runtime: SystemRuntimeInfo,
    dmi: Option<DmiSystemRecord>,
    dmi_source: Option<SourceEvidence>,
) -> Option<Device> {
    if runtime.hostname.is_none()
        && runtime.os.is_none()
        && runtime.kernel.is_none()
        && runtime.architecture.is_none()
        && dmi.is_none()
    {
        return None;
    }

    let dmi = dmi.unwrap_or_default();
    let id = dmi
        .serial
        .as_deref()
        .filter(|serial| !serial.trim().is_empty())
        .map(|serial| format!("system:serial:{serial}"))
        .or_else(|| {
            dmi.uuid
                .as_deref()
                .filter(|uuid| !uuid.trim().is_empty())
                .map(|uuid| format!("system:uuid:{uuid}"))
        })
        .or_else(|| {
            runtime
                .hostname
                .as_deref()
                .map(|hostname| device_id::other("system:hostname", hostname))
        })
        .unwrap_or_else(|| "system:0".to_string());
    let name = dmi
        .product_name
        .clone()
        .or_else(|| runtime.hostname.clone())
        .unwrap_or_else(|| "System".to_string());
    let mut device = Device::new(
        id,
        DeviceKind::System,
        name,
        DeviceProperties::System(SystemDeviceInfo {
            hostname: runtime.hostname,
            os: runtime.os,
            kernel: runtime.kernel,
            architecture: runtime.architecture,
            manufacturer: dmi.manufacturer,
            product_name: dmi.product_name,
            version: dmi.version,
            serial: dmi.serial,
            uuid: dmi.uuid,
            wake_up_type: dmi.wake_up_type,
            sku_number: dmi.sku_number,
            family: dmi.family,
        }),
    );
    if let DeviceProperties::System(info) = &device.properties {
        device.vendor = info.manufacturer.clone();
        device.model = info.product_name.clone();
        device.serial = info.serial.clone();
    }
    for source in runtime.sources {
        device = device.with_source(source);
    }
    if let Some(source) = dmi_source {
        device = device.with_source(source);
    }

    Some(device)
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
    primary.l1d_cache = primary.l1d_cache.or(fallback.l1d_cache);
    primary.l1i_cache = primary.l1i_cache.or(fallback.l1i_cache);
    primary.l2_cache = primary.l2_cache.or(fallback.l2_cache);
    primary.l3_cache = primary.l3_cache.or(fallback.l3_cache);

    Some(primary)
}

async fn read_cpu_cpufreq(ctx: &ProbeContext<'_>) -> Option<hw_parser::CpuRecord> {
    let cpu_max_mhz = read_cpufreq_mhz(
        ctx,
        Path::new("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq"),
    )
    .await;
    let cpu_min_mhz = read_cpufreq_mhz(
        ctx,
        Path::new("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_min_freq"),
    )
    .await;
    let mut cpu_mhz = read_cpufreq_mhz(
        ctx,
        Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq"),
    )
    .await;
    if cpu_mhz.is_none() {
        cpu_mhz = read_cpufreq_mhz(
            ctx,
            Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_setspeed"),
        )
        .await;
    }
    let record = hw_parser::CpuRecord {
        cpu_mhz,
        cpu_max_mhz,
        cpu_min_mhz,
        ..Default::default()
    };

    (!record.is_empty()).then_some(record)
}

async fn read_cpufreq_mhz(ctx: &ProbeContext<'_>, path: &Path) -> Option<u32> {
    let result = ctx.runner.read_file(path).await;
    if result.is_success() {
        parse_cpufreq_khz(&result.stdout)
    } else {
        None
    }
}

fn parse_cpufreq_khz(value: &str) -> Option<u32> {
    let khz = value.trim().parse::<u64>().ok()?;
    let mhz = (khz + 500) / 1000;
    u32::try_from(mhz).ok().filter(|value| *value > 0)
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
    let lshw_storage = storage_lshw_storage_records(ctx).await;
    let lspci = storage_lspci_records(ctx).await;
    let hwinfo = storage_hwinfo_records(ctx).await;

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
        let device = apply_storage_lshw_storage_enrichment(device, &lshw_storage);
        let device = apply_storage_lspci_enrichment(device, &lspci);
        let device = apply_storage_lshw_enrichment(device, &lshw);
        let device = apply_storage_hwinfo_enrichment(device, &hwinfo);
        let device = apply_storage_hdparm_enrichment(ctx, device).await;
        devices.push(apply_storage_smartctl(ctx, device).await);
    }

    if devices.is_empty() {
        storage_devices_from_hwinfo(&hwinfo)
    } else {
        devices
    }
}

#[derive(Default)]
struct StorageLshwRecords {
    source: String,
    by_node: HashMap<String, LshwDiskRecord>,
}

#[derive(Default)]
struct StorageLshwStorageRecords {
    source: String,
    by_pci_address: HashMap<String, LshwStorageRecord>,
}

#[derive(Default)]
struct StorageLspciRecords {
    source: String,
    by_pci_address: HashMap<String, PciRecord>,
}

#[derive(Default)]
struct StorageHwinfoRecords {
    source: String,
    by_node: HashMap<String, HwinfoDiskRecord>,
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

async fn storage_lshw_storage_records(ctx: &ProbeContext<'_>) -> StorageLshwStorageRecords {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("lshw", ["-class", "storage"]),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return StorageLshwStorageRecords::default();
    }

    let by_pci_address = parse_lshw_storage(&result.stdout)
        .into_iter()
        .filter_map(|record| {
            Some((
                pci_address_from_lshw_bus(record.bus_info.as_deref()?)?,
                record,
            ))
        })
        .collect();
    StorageLshwStorageRecords {
        source: result.source,
        by_pci_address,
    }
}

async fn storage_lspci_records(ctx: &ProbeContext<'_>) -> StorageLspciRecords {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("lspci", ["-nn", "-k"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return StorageLspciRecords::default();
    }

    let by_pci_address = parse_lspci_nn_k(&result.stdout)
        .into_iter()
        .map(|record| (record.address.clone(), record))
        .collect();
    StorageLspciRecords {
        source: result.source,
        by_pci_address,
    }
}

async fn storage_hwinfo_records(ctx: &ProbeContext<'_>) -> StorageHwinfoRecords {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("hwinfo", ["--disk"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return StorageHwinfoRecords::default();
    }

    let by_node = parse_hwinfo_disk(&result.stdout)
        .into_iter()
        .filter_map(|record| Some((record.device_node.clone()?, record)))
        .collect();
    StorageHwinfoRecords {
        source: result.source,
        by_node,
    }
}

fn storage_devices_from_hwinfo(hwinfo: &StorageHwinfoRecords) -> Vec<Device> {
    hwinfo
        .by_node
        .values()
        .cloned()
        .map(|record| storage_device_from_hwinfo(record, hwinfo.source.as_str()))
        .collect()
}

fn storage_device_from_hwinfo(record: HwinfoDiskRecord, source: &str) -> Device {
    let node = record.device_node.clone().unwrap_or_default();
    let mut device = Device::new(
        device_id::storage(None, record.serial.as_deref(), &node),
        DeviceKind::Storage,
        record.model.clone().unwrap_or_else(|| node.clone()),
        DeviceProperties::Storage(StorageInfo {
            device_node: Some(node),
            firmware: record.revision.clone(),
            ..Default::default()
        }),
    );
    device = apply_storage_hwinfo_driver(device, &record);
    device.vendor = record.vendor;
    device.model = record.model;
    device.serial = record.serial;
    device.with_source(SourceEvidence {
        source: source.to_string(),
        kind: SourceKind::Command,
        status: SourceStatus::Success,
        summary: None,
    })
}

async fn apply_storage_driver(ctx: &ProbeContext<'_>, device: Device, name: &str) -> Device {
    let sysfs_path = Path::new("/sys/block").join(name);
    let mut device = device;

    if let Some(uevent) = read_optional_trimmed(ctx, &sysfs_path.join("device/uevent")).await {
        let mut contributed = false;
        if let Some(driver) = parse_uevent_value(&uevent, "DRIVER") {
            device = device.with_driver(DriverInfo {
                name: Some(driver),
                version: None,
                modules: Vec::new(),
                provider: None,
                status: DriverStatus::InUse,
            });
            contributed = true;
        }
        if device.bus.is_none() {
            if let Some(bus) = pci_bus_from_uevent(&uevent) {
                device = device.with_bus(bus);
                contributed = true;
            }
        }
        if contributed {
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
        }
    }

    if device.bus.is_none() {
        if let Some(controller) = nvme_controller_name(name) {
            let path = Path::new("/sys/class/nvme")
                .join(controller)
                .join("device/uevent");
            if let Some(uevent) = read_optional_trimmed(ctx, &path).await {
                let mut contributed = false;
                if let Some(bus) = pci_bus_from_uevent(&uevent) {
                    device = device.with_bus(bus);
                    contributed = true;
                }
                if device.driver.is_none() {
                    if let Some(driver) = parse_uevent_value(&uevent, "DRIVER") {
                        device = device.with_driver(DriverInfo {
                            name: Some(driver),
                            version: None,
                            modules: Vec::new(),
                            provider: None,
                            status: DriverStatus::InUse,
                        });
                        contributed = true;
                    }
                }
                if contributed {
                    let source = path.display().to_string();
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
                }
            }
        }
    }

    if device.bus.is_none() {
        device = apply_storage_canonical_pci_identity(ctx, device, &sysfs_path).await;
    }
    if device.bus.is_none() {
        device = apply_storage_parent_pci_identity(ctx, device, &sysfs_path).await;
    }
    if device.bus.is_none() {
        device = apply_unique_storage_controller_pci_identity(ctx, device).await;
    }
    device
}

async fn apply_storage_canonical_pci_identity(
    ctx: &ProbeContext<'_>,
    mut device: Device,
    sysfs_path: &Path,
) -> Device {
    let device_path = sysfs_path.join("device");
    let result = ctx.runner.canonicalize_path(&device_path).await;
    if !result.is_success() {
        return device;
    }
    let Some(address) = pci_address_from_path(Path::new(result.stdout.trim())) else {
        return device;
    };

    let uevent_path = Path::new("/sys/bus/pci/devices")
        .join(&address)
        .join("uevent");
    if let Some(uevent) = read_optional_trimmed(ctx, &uevent_path).await {
        if let Some(bus) = pci_bus_from_uevent(&uevent) {
            device = device.with_bus(bus);
            let source = uevent_path.display().to_string();
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
            return device;
        }
    }

    let source = result.source;
    device = device.with_bus(BusInfo::Pci {
        address,
        vendor_id: None,
        device_id: None,
        subsystem_vendor_id: None,
        subsystem_device_id: None,
        class: None,
    });
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

fn pci_address_from_path(path: &Path) -> Option<String> {
    path.components()
        .rev()
        .filter_map(|component| component.as_os_str().to_str())
        .find(|component| is_pci_address(component))
        .map(str::to_string)
}

async fn apply_storage_parent_pci_identity(
    ctx: &ProbeContext<'_>,
    mut device: Device,
    sysfs_path: &Path,
) -> Device {
    for depth in 1..=6 {
        let mut path = sysfs_path.join("device");
        for _ in 0..depth {
            path = path.join("..");
        }
        let path = path.join("uevent");
        let Some(uevent) = read_optional_trimmed(ctx, &path).await else {
            continue;
        };
        let Some(bus) = pci_bus_from_uevent(&uevent) else {
            continue;
        };
        let source = path.display().to_string();
        device = device.with_bus(bus);
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
        return device;
    }
    device
}

async fn apply_unique_storage_controller_pci_identity(
    ctx: &ProbeContext<'_>,
    mut device: Device,
) -> Device {
    let Some(media_type) = storage_media_type_for_pci_controller(&device) else {
        return device;
    };

    let controllers: Vec<_> = read_sysfs_pci_records(ctx)
        .await
        .into_iter()
        .filter(|record| {
            record
                .class_id
                .as_deref()
                .is_some_and(|class| class.starts_with("01"))
        })
        .collect();
    let matching_indexes: Vec<_> = controllers
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            record
                .class_id
                .as_deref()
                .is_some_and(|class| storage_controller_class_matches_media_type(media_type, class))
                .then_some(index)
        })
        .collect();
    let controller = if let [index] = matching_indexes.as_slice() {
        &controllers[*index]
    } else {
        if !matching_indexes.is_empty() {
            return device;
        }
        let [controller] = controllers.as_slice() else {
            return device;
        };
        controller
    };

    let path = controller.path.join("uevent");
    let Some(uevent) = read_optional_trimmed(ctx, &path).await else {
        return device;
    };
    let Some(bus) = pci_bus_from_uevent(&uevent) else {
        return device;
    };

    let source = path.display().to_string();
    device = device.with_bus(bus);
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

fn storage_media_type_for_pci_controller(device: &Device) -> Option<&str> {
    let DeviceProperties::Storage(storage) = &device.properties else {
        return None;
    };
    storage
        .media_type
        .as_deref()
        .filter(|media_type| matches!(*media_type, "sata" | "ata" | "scsi"))
}

fn storage_controller_class_matches_media_type(media_type: &str, class: &str) -> bool {
    let class = class.trim_start_matches("0x").trim_start_matches("0X");
    match media_type {
        "sata" => class.starts_with("0106"),
        "ata" => class.starts_with("0101") || class.starts_with("0106"),
        "scsi" => class.starts_with("0100") || class.starts_with("0107"),
        _ => false,
    }
}

fn nvme_controller_name(name: &str) -> Option<String> {
    let rest = name.strip_prefix("nvme")?;
    let digit_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digit_count == 0 || !rest[digit_count..].starts_with('n') {
        return None;
    }
    Some(format!("nvme{}", &rest[..digit_count]))
}

fn apply_storage_hwinfo_enrichment(mut device: Device, hwinfo: &StorageHwinfoRecords) -> Device {
    let Some(node) = (match &device.properties {
        DeviceProperties::Storage(storage) => storage.device_node.clone(),
        _ => None,
    }) else {
        return device;
    };
    let Some(record) = hwinfo.by_node.get(&node) else {
        return device;
    };
    let mut contributed = false;

    if device.vendor.is_none() && record.vendor.is_some() {
        device.vendor = record.vendor.clone();
        contributed = true;
    }
    if device.model.is_none() && record.model.is_some() {
        device.model = record.model.clone();
        if device.name == node {
            device.name = record.model.clone().unwrap_or(device.name);
        }
        contributed = true;
    }
    if device.serial.is_none() && record.serial.is_some() {
        device.serial = record.serial.clone();
        contributed = true;
    }
    if let DeviceProperties::Storage(storage) = &mut device.properties {
        if storage.firmware.is_none() && record.revision.is_some() {
            storage.firmware = record.revision.clone();
            contributed = true;
        }
    }
    let had_driver = device.driver.is_some();
    device = apply_storage_hwinfo_driver(device, record);
    contributed |= !had_driver && device.driver.is_some();

    if contributed
        && !hwinfo.source.is_empty()
        && !device
            .sources
            .iter()
            .any(|source| source.source == hwinfo.source)
    {
        device = device.with_source(SourceEvidence {
            source: hwinfo.source.clone(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

fn apply_storage_hwinfo_driver(mut device: Device, record: &HwinfoDiskRecord) -> Device {
    if device.driver.is_some() || (record.driver.is_none() && record.driver_modules.is_empty()) {
        return device;
    }

    device = device.with_driver(DriverInfo {
        name: record.driver.clone(),
        version: None,
        modules: record.driver_modules.clone(),
        provider: None,
        status: DriverStatus::InUse,
    });
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

fn apply_storage_lshw_storage_enrichment(
    mut device: Device,
    lshw: &StorageLshwStorageRecords,
) -> Device {
    let Some(address) = (match device.bus.as_ref() {
        Some(BusInfo::Pci { address, .. }) => Some(address),
        _ => None,
    }) else {
        return device;
    };
    let Some(record) = lshw.by_pci_address.get(address) else {
        return device;
    };
    let mut contributed = false;

    if let DeviceProperties::Storage(storage) = &mut device.properties {
        if storage.controller_vendor.is_none() && record.vendor.is_some() {
            storage.controller_vendor = record.vendor.clone();
            contributed = true;
        }
        if storage.controller_model.is_none() && record.product.is_some() {
            storage.controller_model = record.product.clone();
            contributed = true;
        }
        if storage.controller_driver.is_none() && record.driver.is_some() {
            storage.controller_driver = record.driver.clone();
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

fn apply_storage_lspci_enrichment(mut device: Device, lspci: &StorageLspciRecords) -> Device {
    let Some(address) = (match device.bus.as_ref() {
        Some(BusInfo::Pci { address, .. }) => Some(address),
        _ => None,
    }) else {
        return device;
    };
    let Some(record) = lspci.by_pci_address.get(address) else {
        return device;
    };
    let mut contributed = false;

    if let DeviceProperties::Storage(storage) = &mut device.properties {
        if storage.controller_vendor.is_none() && record.vendor.is_some() {
            storage.controller_vendor = record.vendor.clone();
            contributed = true;
        }
        if storage.controller_model.is_none() && record.device.is_some() {
            storage.controller_model = record.device.clone();
            contributed = true;
        }
        if storage.controller_driver.is_none() && record.kernel_driver.is_some() {
            storage.controller_driver = record.kernel_driver.clone();
            contributed = true;
        }
    }

    if contributed
        && !lspci.source.is_empty()
        && !device
            .sources
            .iter()
            .any(|source| source.source == lspci.source)
    {
        device = device.with_source(SourceEvidence {
            source: lspci.source.clone(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

async fn apply_storage_hdparm_enrichment(ctx: &ProbeContext<'_>, mut device: Device) -> Device {
    let Some(node) = (match &device.properties {
        DeviceProperties::Storage(storage) => storage.device_node.clone(),
        _ => None,
    }) else {
        return device;
    };

    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("hdparm", ["-i".to_string(), node.clone()]),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return device;
    }

    let record = parse_hdparm_identify(&result.stdout);
    let mut contributed = false;
    if device.model.is_none() && record.model.is_some() {
        device.model = record.model.clone();
        if device.name == node {
            device.name = record.model.unwrap_or(device.name);
        }
        contributed = true;
    }
    if device.serial.is_none() && record.serial.is_some() {
        device.serial = record.serial;
        contributed = true;
    }
    if let DeviceProperties::Storage(storage) = &mut device.properties {
        if storage.firmware.is_none() && record.firmware.is_some() {
            storage.firmware = record.firmware;
            contributed = true;
        }
    }

    if contributed {
        device = device.with_source(SourceEvidence {
            source: result.source,
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
    if smart.smart_status.is_none()
        && smart.model.is_none()
        && smart.serial.is_none()
        && smart.firmware.is_none()
        && smart.temperature_celsius.is_none()
        && smart.power_on_hours.is_none()
        && smart.power_cycle_count.is_none()
        && smart.available_spare_percent.is_none()
        && smart.available_spare_threshold_percent.is_none()
        && smart.percentage_used.is_none()
        && smart.data_units_read.is_none()
        && smart.data_units_written.is_none()
        && smart.media_errors.is_none()
        && smart.error_log_entries.is_none()
    {
        return device;
    }
    if let Some(model) = smart.model.clone() {
        device.name = model.clone();
        device.model = Some(model);
    }
    if smart.serial.is_some() {
        device.serial = smart.serial.clone();
    }
    if let DeviceProperties::Storage(storage) = &mut device.properties {
        if storage.firmware.is_none() {
            storage.firmware = smart.firmware.clone();
        }
        storage.smart_status = storage.smart_status.take().or(smart.smart_status);
        storage.temperature_celsius = storage.temperature_celsius.or(smart.temperature_celsius);
        storage.power_on_hours = storage.power_on_hours.or(smart.power_on_hours);
        storage.power_cycle_count = storage.power_cycle_count.or(smart.power_cycle_count);
        storage.available_spare_percent = storage
            .available_spare_percent
            .or(smart.available_spare_percent);
        storage.available_spare_threshold_percent = storage
            .available_spare_threshold_percent
            .or(smart.available_spare_threshold_percent);
        storage.percentage_used = storage.percentage_used.or(smart.percentage_used);
        storage.data_units_read = storage.data_units_read.or(smart.data_units_read);
        storage.data_units_written = storage.data_units_written.or(smart.data_units_written);
        storage.media_errors = storage.media_errors.or(smart.media_errors);
        storage.error_log_entries = storage.error_log_entries.or(smart.error_log_entries);
    }
    if smart.serial.is_some() {
        if let DeviceProperties::Storage(storage) = &device.properties {
            if let Some(node) = storage.device_node.as_deref() {
                device.id =
                    device_id::storage(storage.wwn.as_deref(), device.serial.as_deref(), node);
            }
        }
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

fn pci_address_from_lshw_bus(value: &str) -> Option<String> {
    let address = value.strip_prefix("pci@")?.trim();
    if address.is_empty() {
        return None;
    }
    if !address.contains(':') || address.matches(':').count() == 1 {
        return Some(format!("0000:{address}"));
    }
    Some(address.to_string())
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
        let lshw_storage = storage_lshw_storage_records(ctx).await;
        let lspci = storage_lspci_records(ctx).await;
        let hwinfo = storage_hwinfo_records(ctx).await;
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
            let device = apply_storage_lshw_storage_enrichment(device, &lshw_storage);
            let device = apply_storage_lspci_enrichment(device, &lspci);
            let device = apply_storage_lshw_enrichment(device, &lshw);
            let device = apply_storage_hwinfo_enrichment(device, &hwinfo);
            let device = apply_storage_hdparm_enrichment(ctx, device).await;
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

    let spd_result = ctx
        .runner
        .run_command(
            &CommandSpec::new("decode-dimms", std::iter::empty::<&str>()),
            ctx.timeout,
        )
        .await;
    if spd_result.is_success() {
        let records = parse_spd_decode_dimms(&spd_result.stdout);
        if !records.is_empty() {
            fallback.devices =
                memory_devices_from_records(records, &spd_result.source, SourceKind::Command);
            return fallback;
        }
        fallback.warnings.push(
            ScanWarning::new(
                "source_empty",
                "decode-dimms source produced no DIMM records",
            )
            .with_source(spd_result.source),
        );
    }

    let spd_sysfs_records = memory_records_from_spd_eeprom_sysfs(ctx, &mut fallback.warnings).await;
    if !spd_sysfs_records.is_empty() {
        fallback.devices = memory_devices_from_sysfs_records(spd_sysfs_records);
        return fallback;
    }

    let sysfs_records = memory_records_from_edac_sysfs(ctx).await;
    if !sysfs_records.is_empty() {
        fallback.devices = memory_devices_from_sysfs_records(sysfs_records);
        return fallback;
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

struct SysfsMemoryRecord {
    record: DmiMemoryRecord,
    source: String,
}

async fn memory_records_from_spd_eeprom_sysfs(
    ctx: &ProbeContext<'_>,
    warnings: &mut Vec<ScanWarning>,
) -> Vec<SysfsMemoryRecord> {
    let mut paths = Vec::new();
    for pattern in [
        "/sys/bus/i2c/drivers/eeprom/*/eeprom",
        "/sys/bus/i2c/drivers/ee1004/*/eeprom",
    ] {
        paths.extend(ctx.runner.glob(pattern).await.paths);
    }
    paths.sort();
    paths.dedup();

    let mut records = Vec::new();
    for path in paths {
        let result = ctx.runner.read_file_bytes(&path).await;
        if !result.is_success() {
            warnings.push(source_bytes_failure("memory", &result));
            continue;
        }

        let Some(mut record) = parse_spd_eeprom(&result.bytes) else {
            warnings.push(
                ScanWarning::new(
                    "source_empty",
                    "raw SPD EEPROM source produced no DIMM record",
                )
                .with_source(result.source),
            );
            continue;
        };
        if record.locator.is_none() {
            record.locator = spd_eeprom_locator(&path);
        }
        records.push(SysfsMemoryRecord {
            record,
            source: result.source,
        });
    }

    records
}

fn spd_eeprom_locator(path: &Path) -> Option<String> {
    path.parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .map(str::to_string)
}

async fn memory_records_from_edac_sysfs(ctx: &ProbeContext<'_>) -> Vec<SysfsMemoryRecord> {
    let mut mc_paths = ctx
        .runner
        .glob("/sys/devices/system/edac/mc/mc*")
        .await
        .paths;
    mc_paths.sort();

    let mut records = Vec::new();
    for mc_path in mc_paths {
        let mut dimm_paths = ctx
            .runner
            .glob(&format!("{}/dimm*", mc_path.display()))
            .await
            .paths;
        dimm_paths.sort();

        for dimm_path in dimm_paths {
            let label = read_optional_trimmed(ctx, &dimm_path.join("dimm_label"))
                .await
                .or(read_optional_trimmed(ctx, &dimm_path.join("dimm_location")).await);
            let memory_type = read_optional_trimmed(ctx, &dimm_path.join("dimm_mem_type")).await;
            let size = read_optional_trimmed(ctx, &dimm_path.join("size"))
                .await
                .and_then(edac_size_to_dmi_size);
            let record = DmiMemoryRecord {
                size,
                locator: label,
                memory_type,
                ..Default::default()
            };
            if memory_record_has_data(&record) {
                records.push(SysfsMemoryRecord {
                    record,
                    source: dimm_path.display().to_string(),
                });
            }
        }
    }

    records
}

fn edac_size_to_dmi_size(value: String) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else if value.chars().all(|ch| ch.is_ascii_digit()) {
        Some(format!("{value} MB"))
    } else {
        Some(value.to_string())
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
        || record.total_width.is_some()
        || record.data_width.is_some()
        || record.minimum_voltage.is_some()
        || record.maximum_voltage.is_some()
        || record.configured_voltage.is_some()
}

fn memory_devices_from_sysfs_records(records: Vec<SysfsMemoryRecord>) -> Vec<Device> {
    records
        .into_iter()
        .enumerate()
        .map(|(idx, record)| {
            memory_device_from_record(record.record, idx, &record.source, SourceKind::Sysfs)
        })
        .collect()
}

fn memory_devices_from_records(
    records: Vec<DmiMemoryRecord>,
    source: &str,
    source_kind: SourceKind,
) -> Vec<Device> {
    records
        .into_iter()
        .enumerate()
        .map(|(idx, mem)| memory_device_from_record(mem, idx, source, source_kind))
        .collect()
}

fn memory_device_from_record(
    mem: DmiMemoryRecord,
    idx: usize,
    source: &str,
    source_kind: SourceKind,
) -> Device {
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
            total_width_bits: parse_width_bits(mem.total_width.as_deref()),
            data_width_bits: parse_width_bits(mem.data_width.as_deref()),
            min_voltage_v: parse_voltage_v(mem.minimum_voltage.as_deref()),
            max_voltage_v: parse_voltage_v(mem.maximum_voltage.as_deref()),
            configured_voltage_v: parse_voltage_v(mem.configured_voltage.as_deref()),
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
                let runtime = read_bios_runtime_info(ctx).await;
                fallback.devices = bios_board_devices(
                    dmi,
                    "/sys/class/dmi/id",
                    SourceKind::Sysfs,
                    runtime,
                    None,
                    None,
                );
            }
            return fallback;
        }
        let mut dmi = parse_dmidecode_bios_board(&result.stdout);
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
        let bios_language_source = enrich_dmi_bios_language(ctx, &mut dmi).await;
        let memory_array_source = enrich_dmi_memory_array(ctx, &mut dmi).await;
        let runtime = read_bios_runtime_info(ctx).await;
        ProbeResult::with_devices(bios_board_devices(
            dmi,
            &result.source,
            SourceKind::Command,
            runtime,
            bios_language_source,
            memory_array_source,
        ))
    }
}

async fn enrich_dmi_bios_language(
    ctx: &ProbeContext<'_>,
    dmi: &mut DmiBiosBoardRecord,
) -> Option<SourceEvidence> {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("dmidecode", ["-t", "13"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return None;
    }

    let language = parse_dmidecode_bios_board(&result.stdout);
    let mut contributed = false;
    contributed |= merge_optional_string(
        &mut dmi.bios_language_description_format,
        language.bios_language_description_format,
    );
    if dmi.bios_installable_languages.is_empty() && !language.bios_installable_languages.is_empty()
    {
        dmi.bios_installable_languages = language.bios_installable_languages;
        contributed = true;
    }
    contributed |= merge_optional_string(
        &mut dmi.bios_currently_installed_language,
        language.bios_currently_installed_language,
    );

    contributed.then_some(SourceEvidence {
        source: result.source,
        kind: SourceKind::Command,
        status: SourceStatus::Success,
        summary: None,
    })
}

async fn enrich_dmi_memory_array(
    ctx: &ProbeContext<'_>,
    dmi: &mut DmiBiosBoardRecord,
) -> Option<SourceEvidence> {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("dmidecode", ["-t", "16"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return None;
    }

    let memory_array = parse_dmidecode_bios_board(&result.stdout);
    let mut contributed = false;
    contributed |= merge_optional_string(
        &mut dmi.memory_array_location,
        memory_array.memory_array_location,
    );
    contributed |= merge_optional_string(&mut dmi.memory_array_use, memory_array.memory_array_use);
    contributed |= merge_optional_string(
        &mut dmi.memory_array_error_correction_type,
        memory_array.memory_array_error_correction_type,
    );
    contributed |= merge_optional_string(
        &mut dmi.memory_array_maximum_capacity,
        memory_array.memory_array_maximum_capacity,
    );
    contributed |= merge_optional_string(
        &mut dmi.memory_array_error_information_handle,
        memory_array.memory_array_error_information_handle,
    );
    contributed |= merge_optional_string(
        &mut dmi.memory_array_number_of_devices,
        memory_array.memory_array_number_of_devices,
    );

    contributed.then_some(SourceEvidence {
        source: result.source,
        kind: SourceKind::Command,
        status: SourceStatus::Success,
        summary: None,
    })
}

fn merge_optional_string(target: &mut Option<String>, candidate: Option<String>) -> bool {
    if target.is_none() && candidate.is_some() {
        *target = candidate;
        true
    } else {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct BiosRuntimeInfo {
    firmware_type: Option<String>,
    secure_boot: Option<String>,
    source: Option<String>,
}

async fn read_bios_runtime_info(ctx: &ProbeContext<'_>) -> BiosRuntimeInfo {
    let efi_paths = ctx.runner.glob("/sys/firmware/efi").await.paths;
    if efi_paths.is_empty() {
        return BiosRuntimeInfo {
            firmware_type: Some("bios".to_string()),
            ..Default::default()
        };
    }

    BiosRuntimeInfo {
        firmware_type: Some("uefi".to_string()),
        secure_boot: read_secure_boot_state(ctx).await,
        source: Some("/sys/firmware/efi".to_string()),
    }
}

async fn read_secure_boot_state(ctx: &ProbeContext<'_>) -> Option<String> {
    let mut paths = ctx
        .runner
        .glob("/sys/firmware/efi/efivars/SecureBoot-*")
        .await
        .paths;
    paths.sort();
    let path = paths.first()?;
    let result = ctx.runner.read_file_bytes(path).await;
    if result.is_success() {
        parse_secure_boot_bytes(&result.bytes)
    } else {
        None
    }
}

fn parse_secure_boot_bytes(bytes: &[u8]) -> Option<String> {
    let value = if bytes.len() >= 5 {
        bytes[4]
    } else {
        *bytes.first()?
    };
    match value {
        0 => Some("disabled".to_string()),
        1 => Some("enabled".to_string()),
        _ => None,
    }
}

async fn read_sysfs_dmi(ctx: &ProbeContext<'_>) -> Option<DmiBiosBoardRecord> {
    let dmi = DmiBiosBoardRecord {
        bios_vendor: read_sysfs_dmi_value(ctx, "bios_vendor").await,
        bios_version: read_sysfs_dmi_value(ctx, "bios_version").await,
        bios_release_date: read_sysfs_dmi_value(ctx, "bios_date").await,
        board_manufacturer: read_sysfs_dmi_value(ctx, "board_vendor").await,
        board_product_name: read_sysfs_dmi_value(ctx, "board_name").await,
        board_version: read_sysfs_dmi_value(ctx, "board_version").await,
        board_serial: read_sysfs_dmi_value(ctx, "board_serial").await,
        board_asset_tag: read_sysfs_dmi_value(ctx, "board_asset_tag").await,
        board_location_in_chassis: None,
        board_chassis_handle: None,
        chassis_manufacturer: read_sysfs_dmi_value(ctx, "chassis_vendor").await,
        chassis_type: read_sysfs_dmi_value(ctx, "chassis_type")
            .await
            .map(normalize_sysfs_chassis_type),
        chassis_version: read_sysfs_dmi_value(ctx, "chassis_version").await,
        chassis_serial: read_sysfs_dmi_value(ctx, "chassis_serial").await,
        chassis_asset_tag: read_sysfs_dmi_value(ctx, "chassis_asset_tag").await,
        chassis_boot_up_state: None,
        chassis_power_supply_state: None,
        chassis_thermal_state: None,
        chassis_security_status: None,
        chassis_oem_information: None,
        chassis_height: None,
        chassis_power_cords: None,
        chassis_contained_elements: None,
        chassis_sku_number: None,
        bios_language_description_format: None,
        bios_installable_languages: Vec::new(),
        bios_currently_installed_language: None,
        memory_array_location: None,
        memory_array_use: None,
        memory_array_error_correction_type: None,
        memory_array_maximum_capacity: None,
        memory_array_error_information_handle: None,
        memory_array_number_of_devices: None,
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

fn normalize_sysfs_chassis_type(value: String) -> String {
    match value.trim().parse::<u8>().ok() {
        Some(1) => "Other".to_string(),
        Some(2) => "Unknown".to_string(),
        Some(3) => "Desktop".to_string(),
        Some(4) => "Low Profile Desktop".to_string(),
        Some(5) => "Pizza Box".to_string(),
        Some(6) => "Mini Tower".to_string(),
        Some(7) => "Tower".to_string(),
        Some(8) => "Portable".to_string(),
        Some(9) => "Laptop".to_string(),
        Some(10) => "Notebook".to_string(),
        Some(11) => "Hand Held".to_string(),
        Some(12) => "Docking Station".to_string(),
        Some(13) => "All In One".to_string(),
        Some(14) => "Sub Notebook".to_string(),
        Some(15) => "Space-saving".to_string(),
        Some(16) => "Lunch Box".to_string(),
        Some(17) => "Main Server Chassis".to_string(),
        Some(18) => "Expansion Chassis".to_string(),
        Some(19) => "Sub Chassis".to_string(),
        Some(20) => "Bus Expansion Chassis".to_string(),
        Some(21) => "Peripheral Chassis".to_string(),
        Some(22) => "RAID Chassis".to_string(),
        Some(23) => "Rack Mount Chassis".to_string(),
        Some(24) => "Sealed-case PC".to_string(),
        Some(25) => "Multi-system".to_string(),
        Some(26) => "Compact PCI".to_string(),
        Some(27) => "Advanced TCA".to_string(),
        Some(28) => "Blade".to_string(),
        Some(29) => "Blade Enclosure".to_string(),
        Some(30) => "Tablet".to_string(),
        Some(31) => "Convertible".to_string(),
        Some(32) => "Detachable".to_string(),
        Some(33) => "IoT Gateway".to_string(),
        Some(34) => "Embedded PC".to_string(),
        Some(35) => "Mini PC".to_string(),
        Some(36) => "Stick PC".to_string(),
        _ => value,
    }
}

fn bios_board_devices(
    dmi: DmiBiosBoardRecord,
    source: &str,
    source_kind: SourceKind,
    runtime: BiosRuntimeInfo,
    bios_language_source: Option<SourceEvidence>,
    memory_array_source: Option<SourceEvidence>,
) -> Vec<Device> {
    let mut bios = Device::new(
        "bios:0",
        DeviceKind::Bios,
        dmi.bios_version
            .clone()
            .unwrap_or_else(|| "BIOS".to_string()),
        DeviceProperties::Bios(BiosInfo {
            vendor: dmi.bios_vendor,
            version: dmi.bios_version,
            release_date: dmi.bios_release_date,
            firmware_type: runtime.firmware_type,
            secure_boot: runtime.secure_boot,
            language_description_format: dmi.bios_language_description_format,
            installable_languages: dmi.bios_installable_languages,
            currently_installed_language: dmi.bios_currently_installed_language,
        }),
    )
    .with_source(SourceEvidence {
        source: source.to_string(),
        kind: source_kind,
        status: SourceStatus::Success,
        summary: None,
    });
    if let Some(source) = runtime.source {
        bios = bios.with_source(SourceEvidence {
            source,
            kind: SourceKind::Sysfs,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    if let Some(source) = bios_language_source {
        bios = bios.with_source(source);
    }
    let board = Device::new(
        dmi.board_serial
            .as_ref()
            .map(|serial| format!("motherboard:serial:{serial}"))
            .unwrap_or_else(|| "motherboard:0".to_string()),
        DeviceKind::Motherboard,
        dmi.board_product_name
            .clone()
            .unwrap_or_else(|| "Motherboard".to_string()),
        DeviceProperties::Motherboard(Box::new(MotherboardInfo {
            manufacturer: dmi.board_manufacturer,
            product_name: dmi.board_product_name,
            version: dmi.board_version,
            serial: dmi.board_serial,
            asset_tag: dmi.board_asset_tag,
            location_in_chassis: dmi.board_location_in_chassis,
            chassis_handle: dmi.board_chassis_handle,
            chassis_manufacturer: dmi.chassis_manufacturer,
            chassis_type: dmi.chassis_type,
            chassis_version: dmi.chassis_version,
            chassis_serial: dmi.chassis_serial,
            chassis_asset_tag: dmi.chassis_asset_tag,
            chassis_boot_up_state: dmi.chassis_boot_up_state,
            chassis_power_supply_state: dmi.chassis_power_supply_state,
            chassis_thermal_state: dmi.chassis_thermal_state,
            chassis_security_status: dmi.chassis_security_status,
            chassis_oem_information: dmi.chassis_oem_information,
            chassis_height: dmi.chassis_height,
            chassis_power_cords: dmi.chassis_power_cords,
            chassis_contained_elements: dmi.chassis_contained_elements,
            chassis_sku_number: dmi.chassis_sku_number,
            memory_array_location: dmi.memory_array_location,
            memory_array_use: dmi.memory_array_use,
            memory_array_error_correction_type: dmi.memory_array_error_correction_type,
            memory_array_maximum_capacity: dmi.memory_array_maximum_capacity,
            memory_array_error_information_handle: dmi.memory_array_error_information_handle,
            memory_array_number_of_devices: dmi.memory_array_number_of_devices,
        })),
    )
    .with_source(SourceEvidence {
        source: source.to_string(),
        kind: source_kind,
        status: SourceStatus::Success,
        summary: None,
    });
    let board = if let Some(source) = memory_array_source {
        board.with_source(source)
    } else {
        board
    };
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
        let drm = gpu_drm_records(ctx).await;
        let dmesg = gpu_dmesg_records(ctx).await;
        let nvidia_smi = gpu_nvidia_smi_records(ctx).await;
        let nvidia_settings = gpu_nvidia_settings_record(ctx).await;
        let glxinfo = gpu_glxinfo_record(ctx).await;
        let proc_gpuinfo = gpu_proc_gpuinfo_record(ctx).await;
        let enrichments = GpuEnrichmentSources {
            lshw: &lshw,
            drm: &drm,
            dmesg: &dmesg,
            nvidia_smi: &nvidia_smi,
            nvidia_settings: &nvidia_settings,
            proc_gpuinfo: &proc_gpuinfo,
        };
        let result = ctx
            .runner
            .run_command(&CommandSpec::new("lspci", ["-nn", "-k"]), ctx.timeout)
            .await;
        if !result.is_success() {
            let mut fallback = ProbeResult::source_failure(self.name(), &result);
            fallback.devices =
                gpu_devices_from_sysfs_pci(ctx, &mut fallback.consumed, &enrichments).await;
            fallback.devices = apply_gpu_glxinfo_to_devices(fallback.devices, &glxinfo);
            return fallback;
        }
        let mut probe_result = ProbeResult::default();
        let gpus = parse_gpu_lspci(&result.stdout);
        let jingjia_gpu_count = gpus
            .iter()
            .filter(|gpu| is_jingjia_vendor_id(gpu.vendor_id.as_deref()))
            .count();
        let nvidia_gpu_count = gpus
            .iter()
            .filter(|gpu| {
                is_nvidia_gpu_identity(
                    gpu.vendor_id.as_deref(),
                    gpu.vendor.as_deref(),
                    gpu.device.as_deref(),
                )
            })
            .count();
        for gpu in gpus {
            let address = gpu.address.clone();
            let use_nvidia_settings = nvidia_gpu_count == 1
                && is_nvidia_gpu_identity(
                    gpu.vendor_id.as_deref(),
                    gpu.vendor.as_deref(),
                    gpu.device.as_deref(),
                );
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
            let sysfs_gpu_info = gpu_sysfs_gpu_info_record(ctx, &gpu.address).await;
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
            probe_result.devices.push(apply_gpu_proc_gpuinfo_enrichment(
                apply_gpu_memory_enrichment(
                    apply_gpu_drm_enrichment(
                        apply_gpu_lshw_enrichment(
                            apply_gpu_dmesg_enrichment(device, &dmesg),
                            &lshw,
                        ),
                        &drm,
                    ),
                    sysfs_gpu_info
                        .as_ref()
                        .or_else(|| gpu_nvidia_smi_record(&nvidia_smi, &address))
                        .or_else(|| {
                            unique_nvidia_settings_record(&nvidia_settings, use_nvidia_settings)
                        }),
                ),
                &proc_gpuinfo,
                jingjia_gpu_count == 1,
            ));
        }
        probe_result.devices = apply_gpu_glxinfo_to_devices(probe_result.devices, &glxinfo);
        probe_result
    }
}

#[derive(Default)]
struct GpuLshwDisplayRecords {
    source: String,
    by_pci_address: HashMap<String, LshwDisplayRecord>,
}

#[derive(Default)]
struct GpuDrmRecords {
    by_pci_address: HashMap<String, GpuDrmRecord>,
}

#[derive(Default)]
struct GpuDmesgRecords {
    source: String,
    by_pci_address: HashMap<String, DmesgGpuVramRecord>,
}

#[derive(Default)]
struct GpuNvidiaSmiRecords {
    by_pci_address: HashMap<String, GpuMemoryRecord>,
}

#[derive(Default)]
struct GpuGlxinfoRecord {
    source: String,
    record: GlxinfoBasicRecord,
}

struct GpuMemoryRecord {
    memory_bytes: u64,
    source: String,
    kind: SourceKind,
}

struct GpuEnrichmentSources<'a> {
    lshw: &'a GpuLshwDisplayRecords,
    drm: &'a GpuDrmRecords,
    dmesg: &'a GpuDmesgRecords,
    nvidia_smi: &'a GpuNvidiaSmiRecords,
    nvidia_settings: &'a Option<GpuMemoryRecord>,
    proc_gpuinfo: &'a Option<GpuMemoryRecord>,
}

struct GpuDrmRecord {
    memory_bytes: u64,
    source: String,
}

async fn gpu_glxinfo_record(ctx: &ProbeContext<'_>) -> GpuGlxinfoRecord {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("glxinfo", ["-B"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return GpuGlxinfoRecord::default();
    }

    GpuGlxinfoRecord {
        source: result.source,
        record: parse_glxinfo_basic(&result.stdout),
    }
}

fn apply_gpu_glxinfo_to_devices(
    mut devices: Vec<Device>,
    glxinfo: &GpuGlxinfoRecord,
) -> Vec<Device> {
    if devices.len() == 1 {
        let device = devices.remove(0);
        return vec![apply_gpu_glxinfo_enrichment(device, glxinfo)];
    }
    let Some(index) = unique_glxinfo_gpu_index(&devices, &glxinfo.record) else {
        return devices;
    };
    let device = devices.remove(index);
    devices.insert(index, apply_gpu_glxinfo_enrichment(device, glxinfo));
    devices
}

fn unique_glxinfo_gpu_index(devices: &[Device], record: &GlxinfoBasicRecord) -> Option<usize> {
    unique_glxinfo_gpu_model_index(devices, record)
        .or_else(|| unique_glxinfo_gpu_vendor_index(devices, record))
}

fn unique_glxinfo_gpu_model_index(
    devices: &[Device],
    record: &GlxinfoBasicRecord,
) -> Option<usize> {
    let renderer = record.renderer.as_deref()?;
    let renderer_tokens = gpu_match_tokens(renderer);
    let mut matches = devices.iter().enumerate().filter_map(|(index, device)| {
        device_matches_glxinfo_model(device, &renderer_tokens).then_some(index)
    });
    let index = matches.next()?;
    matches.next().is_none().then_some(index)
}

fn unique_glxinfo_gpu_vendor_index(
    devices: &[Device],
    record: &GlxinfoBasicRecord,
) -> Option<usize> {
    let mut matches = devices
        .iter()
        .enumerate()
        .filter_map(|(index, device)| device_matches_glxinfo(device, record).then_some(index));
    let index = matches.next()?;
    matches.next().is_none().then_some(index)
}

fn device_matches_glxinfo_model(device: &Device, renderer_tokens: &[String]) -> bool {
    gpu_identity_strings(device)
        .into_iter()
        .any(|identity| gpu_identity_matches_renderer(&identity, renderer_tokens))
}

fn gpu_identity_strings(device: &Device) -> Vec<String> {
    let mut identities = vec![device.name.clone()];
    if let Some(model) = &device.model {
        identities.push(model.clone());
    }
    identities
}

fn gpu_identity_matches_renderer(identity: &str, renderer_tokens: &[String]) -> bool {
    let identity_tokens = gpu_match_tokens(identity);
    for len in 2..=identity_tokens.len().min(5) {
        for window in identity_tokens.windows(len) {
            if !window
                .iter()
                .any(|token| token.chars().any(|ch| ch.is_ascii_digit()))
            {
                continue;
            }
            if renderer_tokens
                .windows(window.len())
                .any(|candidate| candidate == window)
            {
                return true;
            }
        }
    }
    false
}

fn gpu_match_tokens(value: &str) -> Vec<String> {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn device_matches_glxinfo(device: &Device, record: &GlxinfoBasicRecord) -> bool {
    normalized_device_gpu_vendor(device)
        .zip(normalized_glxinfo_vendor(record))
        .is_some_and(|(device_vendor, glxinfo_vendor)| device_vendor == glxinfo_vendor)
}

fn normalized_device_gpu_vendor(device: &Device) -> Option<&'static str> {
    if let Some(vendor) = device.bus.as_ref().and_then(|bus| match bus {
        BusInfo::Pci { vendor_id, .. } => vendor_id.as_deref().and_then(normalize_gpu_vendor_id),
        _ => None,
    }) {
        return Some(vendor);
    }
    if let DeviceProperties::Gpu(gpu) = &device.properties {
        if let Some(vendor) = gpu.vendor.as_deref().and_then(normalize_gpu_vendor) {
            return Some(vendor);
        }
    }
    device
        .vendor
        .as_deref()
        .and_then(normalize_gpu_vendor)
        .or_else(|| normalize_gpu_vendor(&device.name))
        .or_else(|| device.model.as_deref().and_then(normalize_gpu_vendor))
}

fn normalized_glxinfo_vendor(record: &GlxinfoBasicRecord) -> Option<&'static str> {
    record
        .vendor
        .as_deref()
        .and_then(normalize_gpu_vendor)
        .or_else(|| record.renderer.as_deref().and_then(normalize_gpu_vendor))
}

fn apply_gpu_glxinfo_enrichment(mut device: Device, glxinfo: &GpuGlxinfoRecord) -> Device {
    let record = &glxinfo.record;
    let mut contributed = false;

    if let Some(renderer) = record.renderer.clone() {
        device.name = renderer.clone();
        device.model = Some(renderer);
        contributed = true;
    }
    if device.vendor.is_none() {
        if let Some(vendor) = record.vendor.clone() {
            device.vendor = Some(vendor);
            contributed = true;
        }
    }
    if let DeviceProperties::Gpu(gpu) = &mut device.properties {
        if gpu.renderer.is_none() && record.renderer.is_some() {
            gpu.renderer = record.renderer.clone();
            contributed = true;
        }
        if gpu.opengl_vendor.is_none() && record.vendor.is_some() {
            gpu.opengl_vendor = record.vendor.clone();
            contributed = true;
        }
        if gpu.opengl_version.is_none() && record.version.is_some() {
            gpu.opengl_version = record.version.clone();
            contributed = true;
        }
    }

    if contributed
        && !glxinfo.source.is_empty()
        && !device
            .sources
            .iter()
            .any(|source| source.source == glxinfo.source)
    {
        device = device.with_source(SourceEvidence {
            source: glxinfo.source.clone(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

async fn gpu_sysfs_gpu_info_record(
    ctx: &ProbeContext<'_>,
    address: &str,
) -> Option<GpuMemoryRecord> {
    let path = Path::new("/sys/bus/pci/devices")
        .join(address)
        .join("gpu-info");
    let result = ctx.runner.read_file(&path).await;
    if !result.is_success() {
        return None;
    }
    Some(GpuMemoryRecord {
        memory_bytes: parse_deepin_gpu_info_vram_total(&result.stdout)?,
        source: result.source,
        kind: SourceKind::Sysfs,
    })
}

async fn gpu_proc_gpuinfo_record(ctx: &ProbeContext<'_>) -> Option<GpuMemoryRecord> {
    let result = ctx.runner.read_file(Path::new("/proc/gpuinfo_0")).await;
    if !result.is_success() {
        return None;
    }
    Some(GpuMemoryRecord {
        memory_bytes: parse_proc_gpuinfo_memory_size(&result.stdout)?,
        source: result.source,
        kind: SourceKind::Procfs,
    })
}

fn parse_deepin_gpu_info_vram_total(input: &str) -> Option<u64> {
    input.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if key.trim() != "VRAM total size" {
            return None;
        }
        let value = value.trim().trim_start_matches("0x");
        u64::from_str_radix(value, 16).ok()
    })
}

fn parse_proc_gpuinfo_memory_size(input: &str) -> Option<u64> {
    input.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if key.trim() == "Memory Size" {
            parse_size_to_bytes(Some(value.trim()))
        } else {
            None
        }
    })
}

fn apply_gpu_proc_gpuinfo_enrichment(
    device: Device,
    proc_gpuinfo: &Option<GpuMemoryRecord>,
    has_unique_jingjia_gpu: bool,
) -> Device {
    if !has_unique_jingjia_gpu || !device_is_jingjia_gpu(&device) {
        return device;
    }
    apply_gpu_memory_enrichment(device, proc_gpuinfo.as_ref())
}

fn apply_gpu_memory_enrichment(mut device: Device, record: Option<&GpuMemoryRecord>) -> Device {
    let Some(record) = record else {
        return device;
    };
    let mut contributed = false;

    if let DeviceProperties::Gpu(gpu) = &mut device.properties {
        if gpu.memory_bytes.is_none() {
            gpu.memory_bytes = Some(record.memory_bytes);
            contributed = true;
        }
    }

    if contributed
        && !device
            .sources
            .iter()
            .any(|source| source.source == record.source)
    {
        device = device.with_source(SourceEvidence {
            source: record.source.clone(),
            kind: record.kind,
            status: SourceStatus::Success,
            summary: None,
        });
    }

    device
}

fn device_is_jingjia_gpu(device: &Device) -> bool {
    device
        .bus
        .as_ref()
        .and_then(|bus| match bus {
            BusInfo::Pci { vendor_id, .. } => vendor_id.as_deref(),
            _ => None,
        })
        .is_some_and(|vendor_id| is_jingjia_vendor_id(Some(vendor_id)))
}

fn is_jingjia_vendor_id(vendor_id: Option<&str>) -> bool {
    vendor_id.is_some_and(|vendor_id| vendor_id.eq_ignore_ascii_case("0731"))
}

fn is_nvidia_gpu_identity(
    vendor_id: Option<&str>,
    vendor: Option<&str>,
    device: Option<&str>,
) -> bool {
    vendor_id
        .and_then(normalize_gpu_vendor_id)
        .is_some_and(|vendor| vendor == "NVIDIA")
        || vendor
            .and_then(normalize_gpu_vendor)
            .is_some_and(|vendor| vendor == "NVIDIA")
        || device
            .and_then(normalize_gpu_vendor)
            .is_some_and(|vendor| vendor == "NVIDIA")
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

async fn gpu_dmesg_records(ctx: &ProbeContext<'_>) -> GpuDmesgRecords {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("dmesg", std::iter::empty::<&str>()),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return GpuDmesgRecords::default();
    }

    let by_pci_address = parse_dmesg_gpu_vram(&result.stdout)
        .into_iter()
        .map(|record| (record.pci_address.clone(), record))
        .collect();
    GpuDmesgRecords {
        source: result.source,
        by_pci_address,
    }
}

fn apply_gpu_dmesg_enrichment(mut device: Device, dmesg: &GpuDmesgRecords) -> Device {
    let Some(address) = device.bus.as_ref().and_then(gpu_pci_address) else {
        return device;
    };
    let Some(record) = dmesg.by_pci_address.get(address) else {
        return device;
    };
    let mut contributed = false;

    if let DeviceProperties::Gpu(gpu) = &mut device.properties {
        if gpu.memory_bytes.is_none() {
            gpu.memory_bytes = Some(record.memory_bytes);
            contributed = true;
        }
    }

    if contributed
        && !dmesg.source.is_empty()
        && !device
            .sources
            .iter()
            .any(|source| source.source == dmesg.source)
    {
        device = device.with_source(SourceEvidence {
            source: dmesg.source.clone(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }

    device
}

async fn gpu_nvidia_smi_records(ctx: &ProbeContext<'_>) -> GpuNvidiaSmiRecords {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new(
                "nvidia-smi",
                [
                    "--query-gpu=pci.bus_id,memory.total",
                    "--format=csv,noheader,nounits",
                ],
            ),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return GpuNvidiaSmiRecords::default();
    }

    let by_pci_address = parse_nvidia_smi_memory_csv(&result.stdout)
        .into_iter()
        .map(|record| {
            (
                record.pci_address,
                GpuMemoryRecord {
                    memory_bytes: record.memory_bytes,
                    source: result.source.clone(),
                    kind: SourceKind::Command,
                },
            )
        })
        .collect();
    GpuNvidiaSmiRecords { by_pci_address }
}

fn gpu_nvidia_smi_record<'a>(
    records: &'a GpuNvidiaSmiRecords,
    address: &str,
) -> Option<&'a GpuMemoryRecord> {
    records.by_pci_address.get(address)
}

async fn gpu_nvidia_settings_record(ctx: &ProbeContext<'_>) -> Option<GpuMemoryRecord> {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("nvidia-settings", ["-q", "VideoRam"]),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return None;
    }

    Some(GpuMemoryRecord {
        memory_bytes: parse_nvidia_settings_videoram(&result.stdout)?,
        source: result.source,
        kind: SourceKind::Command,
    })
}

fn unique_nvidia_settings_record(
    record: &Option<GpuMemoryRecord>,
    use_for_device: bool,
) -> Option<&GpuMemoryRecord> {
    use_for_device.then_some(record.as_ref()).flatten()
}

async fn gpu_drm_records(ctx: &ProbeContext<'_>) -> GpuDrmRecords {
    let mut records = GpuDrmRecords::default();
    let mut paths = ctx
        .runner
        .glob("/sys/class/drm/*/device/uevent")
        .await
        .paths;
    paths.sort();

    for uevent_path in paths {
        let result = ctx.runner.read_file(&uevent_path).await;
        if !result.is_success() {
            continue;
        }
        let Some(address) = pci_bus_from_uevent(&result.stdout).and_then(|bus| match bus {
            BusInfo::Pci { address, .. } => Some(address),
            _ => None,
        }) else {
            continue;
        };
        let Some(device_path) = uevent_path.parent() else {
            continue;
        };
        let memory_path = device_path.join("mem_info_vram_total");
        let Some(memory_bytes) = read_optional_trimmed(ctx, &memory_path)
            .await
            .and_then(|value| value.parse::<u64>().ok())
        else {
            continue;
        };

        records.by_pci_address.insert(
            address,
            GpuDrmRecord {
                memory_bytes,
                source: memory_path.display().to_string(),
            },
        );
    }

    records
}

fn apply_gpu_drm_enrichment(mut device: Device, drm: &GpuDrmRecords) -> Device {
    let Some(address) = device.bus.as_ref().and_then(gpu_pci_address) else {
        return device;
    };
    let Some(record) = drm.by_pci_address.get(address) else {
        return device;
    };
    let mut contributed = false;

    if let DeviceProperties::Gpu(gpu) = &mut device.properties {
        if gpu.memory_bytes.is_none() {
            gpu.memory_bytes = Some(record.memory_bytes);
            contributed = true;
        }
    }

    if contributed
        && !device
            .sources
            .iter()
            .any(|source| source.source == record.source)
    {
        device = device.with_source(SourceEvidence {
            source: record.source.clone(),
            kind: SourceKind::Sysfs,
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
    sources: &GpuEnrichmentSources<'_>,
) -> Vec<Device> {
    let mut devices = Vec::new();
    let records: Vec<_> = read_sysfs_pci_records(ctx)
        .await
        .into_iter()
        .filter(|record| {
            record
                .class_id
                .as_deref()
                .is_some_and(|class| class.starts_with("03"))
        })
        .collect();
    let jingjia_gpu_count = records
        .iter()
        .filter(|record| is_jingjia_vendor_id(record.vendor_id.as_deref()))
        .count();
    let nvidia_gpu_count = records
        .iter()
        .filter(|record| is_nvidia_gpu_identity(record.vendor_id.as_deref(), None, None))
        .count();

    for record in records {
        let address = record.address.clone();
        let use_nvidia_settings = nvidia_gpu_count == 1
            && is_nvidia_gpu_identity(record.vendor_id.as_deref(), None, None);
        let vendor = record
            .vendor_id
            .as_deref()
            .and_then(normalize_gpu_vendor_id)
            .map(str::to_string);

        consumed.push(DeviceRef {
            id: device_id::pci(&record.address),
        });
        let sysfs_gpu_info = gpu_sysfs_gpu_info_record(ctx, &record.address).await;
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
        devices.push(apply_gpu_proc_gpuinfo_enrichment(
            apply_gpu_memory_enrichment(
                apply_gpu_drm_enrichment(
                    apply_gpu_lshw_enrichment(
                        apply_gpu_dmesg_enrichment(device, sources.dmesg),
                        sources.lshw,
                    ),
                    sources.drm,
                ),
                sysfs_gpu_info
                    .as_ref()
                    .or_else(|| gpu_nvidia_smi_record(sources.nvidia_smi, &address))
                    .or_else(|| {
                        unique_nvidia_settings_record(sources.nvidia_settings, use_nvidia_settings)
                    }),
            ),
            sources.proc_gpuinfo,
            jingjia_gpu_count == 1,
        ));
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
        let hwinfo = monitor_hwinfo_records(ctx).await;

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
            if paths.len() == 1 {
                let bytes_result = ctx.runner.read_file_bytes(&paths[0]).await;
                if bytes_result.is_success() {
                    edids
                        .entry(connector)
                        .or_default()
                        .push((bytes_result.bytes, bytes_result.source));
                } else {
                    warnings.push(source_bytes_failure(self.name(), &bytes_result));
                }
                continue;
            }

            let mut readable = Vec::new();
            for path in paths {
                let bytes_result = ctx.runner.read_file_bytes(&path).await;
                if bytes_result.is_success() {
                    readable.push((bytes_result.bytes, bytes_result.source, path));
                }
            }
            if readable.len() == 1 {
                let (bytes, source, _) = readable.remove(0);
                edids.entry(connector).or_default().push((bytes, source));
                continue;
            }

            let mut connected = Vec::new();
            for (index, (_, _, path)) in readable.iter().enumerate() {
                let Some(connector_path) = path.parent() else {
                    continue;
                };
                let Some(status) = read_optional_trimmed(ctx, &connector_path.join("status")).await
                else {
                    continue;
                };
                if status.eq_ignore_ascii_case("connected") {
                    connected.push(index);
                }
            }
            if connected.len() == 1 {
                let (bytes, source, _) = readable.swap_remove(connected[0]);
                edids.entry(connector).or_default().push((bytes, source));
                continue;
            }

            let mut enabled = Vec::new();
            for (index, (_, _, path)) in readable.iter().enumerate() {
                let Some(connector_path) = path.parent() else {
                    continue;
                };
                let Some(enabled_state) =
                    read_optional_trimmed(ctx, &connector_path.join("enabled")).await
                else {
                    continue;
                };
                if enabled_state.eq_ignore_ascii_case("enabled") {
                    enabled.push(index);
                }
            }
            if enabled.len() == 1 {
                let (bytes, source, _) = readable.swap_remove(enabled[0]);
                edids.entry(connector).or_default().push((bytes, source));
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
                        mon.max_resolution,
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
                        None,
                        result.source.clone(),
                        SourceKind::Command,
                        true,
                    )
                })
                .collect()
        };
        monitors.sort_by(|left, right| left.0.cmp(&right.0));

        let mut devices: Vec<_> = monitors
            .into_iter()
            .filter_map(
                |(
                    connector,
                    resolution,
                    max_resolution,
                    mut source,
                    mut source_kind,
                    require_edid,
                )| {
                    let id = device_id::other("monitor", &connector);
                    let mut info = MonitorInfo {
                        connector: Some(connector.clone()),
                        resolution,
                        max_resolution,
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

                    let device = Device::new(
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
                    });
                    Some(apply_monitor_hwinfo_enrichment(device, &hwinfo))
                },
            )
            .collect();
        if devices.is_empty() {
            devices = hwinfo
                .records
                .iter()
                .enumerate()
                .map(|(index, record)| monitor_device_from_hwinfo(index, record, &hwinfo.source))
                .collect();
        }
        ProbeResult {
            devices,
            warnings,
            consumed: Vec::new(),
        }
    }
}

fn monitor_device_from_hwinfo(index: usize, record: &HwinfoMonitorRecord, source: &str) -> Device {
    let key = record
        .serial
        .clone()
        .or_else(|| record.model.clone())
        .unwrap_or_else(|| index.to_string());
    let name = record
        .model
        .clone()
        .or_else(|| record.vendor.clone())
        .unwrap_or_else(|| "Monitor".to_string());
    Device::new(
        device_id::other("monitor:hwinfo", &key),
        DeviceKind::Monitor,
        name,
        DeviceProperties::Monitor(MonitorInfo {
            resolution: record.resolution.clone(),
            size_mm: record.size_mm,
            manufacturer: record.vendor.clone(),
            manufacturer_name: record.vendor.clone(),
            product: record.model.clone(),
            serial: record.serial.clone(),
            ..Default::default()
        }),
    )
    .with_source(SourceEvidence {
        source: source.to_string(),
        kind: SourceKind::Command,
        status: SourceStatus::Success,
        summary: None,
    })
}

#[derive(Default)]
struct MonitorHwinfoRecords {
    source: String,
    records: Vec<HwinfoMonitorRecord>,
}

async fn monitor_hwinfo_records(ctx: &ProbeContext<'_>) -> MonitorHwinfoRecords {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("hwinfo", ["--monitor"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return MonitorHwinfoRecords::default();
    }

    MonitorHwinfoRecords {
        source: result.source,
        records: parse_hwinfo_monitor(&result.stdout),
    }
}

fn apply_monitor_hwinfo_enrichment(mut device: Device, hwinfo: &MonitorHwinfoRecords) -> Device {
    let DeviceProperties::Monitor(info) = &mut device.properties else {
        return device;
    };
    let Some(record) = matching_hwinfo_monitor(info, &hwinfo.records) else {
        return device;
    };

    if info.product.is_none() {
        info.product = record.model.clone();
    }
    if info.manufacturer.is_none() {
        info.manufacturer = record.vendor.clone();
    }
    if info.manufacturer_name.is_none() {
        info.manufacturer_name = record.vendor.clone();
    }
    if info.serial.is_none() {
        info.serial = record.serial.clone();
    }
    if info.size_mm.is_none() {
        info.size_mm = record.size_mm;
    }
    if info.resolution.is_none() {
        info.resolution = record.resolution.clone();
    }
    if !hwinfo.source.is_empty()
        && !device
            .sources
            .iter()
            .any(|source| source.source == hwinfo.source)
    {
        device.sources.push(SourceEvidence {
            source: hwinfo.source.clone(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }

    device
}

fn matching_hwinfo_monitor<'a>(
    info: &MonitorInfo,
    records: &'a [HwinfoMonitorRecord],
) -> Option<&'a HwinfoMonitorRecord> {
    if records.len() == 1 {
        return records.first();
    }
    let resolution = info.resolution.as_deref()?;
    let mut matches = records
        .iter()
        .filter(|record| record.resolution.as_deref() == Some(resolution));
    let matched = matches.next()?;
    matches.next().is_none().then_some(matched)
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
    info.diagonal_inches = edid.size_cm.map(diagonal_inches);
    info.gamma = edid.gamma;
    info.preferred_width = edid.preferred_mode.as_ref().map(|mode| mode.width);
    info.preferred_height = edid.preferred_mode.as_ref().map(|mode| mode.height);
    info.preferred_refresh_hz = edid.preferred_mode.as_ref().map(|mode| mode.refresh_hz);
}

fn diagonal_inches((width_cm, height_cm): (u8, u8)) -> f32 {
    let diagonal_cm = ((width_cm as f32).powi(2) + (height_cm as f32).powi(2)).sqrt();
    (diagonal_cm / 2.54 * 10.0).round() / 10.0
}

fn normalize_sysfs_connector(path: &Path) -> Option<String> {
    let name = path.parent()?.file_name()?.to_str()?;
    let connector = name
        .strip_prefix("card")
        .and_then(|rest| rest.split_once('-').map(|(_, connector)| connector))
        .unwrap_or(name);
    Some(connector.replace("HDMI-A-", "HDMI-"))
}
