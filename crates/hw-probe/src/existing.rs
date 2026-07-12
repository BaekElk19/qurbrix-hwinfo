use crate::{
    sysfs_pci::{is_pci_address, pci_bus_from_uevent, read_kernel_modules, read_sysfs_pci_records},
    Probe, ProbeContext, ProbeResult,
};
use async_trait::async_trait;
use hw_model::{
    device_id, BiosInfo, BusInfo, CpuInfo, Device, DeviceKind, DeviceProperties, DeviceRef,
    DriverInfo, DriverStatus, GpuConnectorInfo, GpuInfo, MemoryInfo, MonitorInfo, MotherboardInfo,
    NetworkInfo, ScanWarning, SourceEvidence, SourceKind, SourceStatus, StorageInfo,
    SystemDeviceInfo,
};
use hw_parser::{
    cpu_extensions_from_flags, infer_cpu_vendor_from_name, lookup_pnp_manufacturer,
    merge_cpu_records, normalize_arch, normalize_board_vendor, normalize_cpu_vendor_id,
    normalize_gpu_vendor, normalize_gpu_vendor_id, parse_dmesg_gpu_vram,
    parse_dmidecode_bios_board, parse_dmidecode_memory, parse_dmidecode_processor,
    parse_dmidecode_system, parse_edid, parse_glxinfo_basic, parse_gpu_lspci,
    parse_hdparm_identify, parse_hwinfo_disk, parse_hwinfo_monitor, parse_ip_j_addr_result,
    parse_ip_j_link_result, parse_lsblk_json_result, parse_lscpu, parse_lshw_disk,
    parse_lshw_display, parse_lshw_memory, parse_lshw_network, parse_lshw_processor,
    parse_lshw_storage, parse_lspci_host_bridge_chipset, parse_lspci_nn_k,
    parse_dmi_oem_strings, parse_modinfo_version, parse_nvidia_settings_memory_interface,
    parse_nvidia_settings_videoram, parse_nvidia_smi_memory_csv, parse_phytium1500a_info,
    parse_proc_cpuinfo, parse_proc_hardware,
    parse_proc_meminfo_total_bytes, parse_size_to_bytes, parse_smartctl_json,
    parse_spd_decode_dimms, parse_spd_eeprom, parse_speed_mtps, parse_voltage_v, parse_width_bits,
    parse_xrandr_query, parse_xrandr_verbose, DmesgGpuVramRecord, DmiBiosBoardRecord,
    DmiMemoryRecord, DmiSystemRecord, GlxinfoBasicRecord, HwinfoDiskRecord, HwinfoMonitorRecord,
    LshwDiskRecord, LshwDisplayRecord, LshwNetworkRecord, LshwStorageRecord, PciRecord,
    XrandrMonitorRecord,
};
use hw_source::{CommandSpec, SourceBytesResult, SourceErrorKind, SourceResult};
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
        let phytium1500a_result = ctx
            .runner
            .read_file(Path::new("/sys/phytium1500a_info"))
            .await;
        let sensors_result = ctx
            .runner
            .run_command(
                &CommandSpec::new("sensors", std::iter::empty::<&str>()),
                ctx.timeout,
            )
            .await;
        let cpu_sysfs = read_cpu_sysfs(ctx).await;
        let cpufreq = read_cpu_cpufreq(ctx).await;

        let mut warnings = Vec::new();
        let lscpu = if lscpu_result.is_success() {
            let record = parse_lscpu(&lscpu_result.stdout);
            if record.is_empty() {
                warnings.push(
                    ScanWarning::new("source_empty", "cpu source produced no lscpu fields")
                        .with_source(lscpu_result.source.clone()),
                );
                None
            } else {
                Some(record)
            }
        } else {
            warnings.extend(ProbeResult::source_failure(self.name(), &lscpu_result).warnings);
            None
        };
        let lshw = if lshw_result.is_success() {
            let record = parse_lshw_processor(&lshw_result.stdout);
            if record.is_empty() {
                warnings.push(
                    ScanWarning::new(
                        "source_empty",
                        "cpu source produced no lshw processor fields",
                    )
                    .with_source(lshw_result.source.clone()),
                );
                None
            } else {
                Some(record)
            }
        } else {
            warnings.extend(ProbeResult::source_failure(self.name(), &lshw_result).warnings);
            None
        };
        let dmi = if dmi_result.is_success() {
            let records = parse_dmidecode_processor(&dmi_result.stdout);
            if !records.iter().any(hw_parser::DmidecodeCpuRecord::is_useful) {
                warnings.push(
                    ScanWarning::new(
                        "source_empty",
                        "cpu source produced no DMI processor records",
                    )
                    .with_source(dmi_result.source.clone()),
                );
            }
            records
        } else {
            warnings.extend(ProbeResult::source_failure(self.name(), &dmi_result).warnings);
            Vec::new()
        };
        let proc_cpuinfo = if proc_cpuinfo_result.is_success() {
            let record = parse_proc_cpuinfo(&proc_cpuinfo_result.stdout);
            if record.is_empty() {
                warnings.push(
                    ScanWarning::new("source_empty", "cpu source produced no proc cpuinfo fields")
                        .with_source(proc_cpuinfo_result.source.clone()),
                );
                None
            } else {
                Some(record)
            }
        } else {
            None
        };
        let proc_hardware = if proc_hardware_result.is_success() {
            let record = parse_proc_hardware(&proc_hardware_result.stdout);
            if record.is_empty() {
                warnings.push(
                    ScanWarning::new(
                        "source_empty",
                        "cpu source produced no proc hardware fields",
                    )
                    .with_source(proc_hardware_result.source.clone()),
                );
                None
            } else {
                Some(record)
            }
        } else {
            None
        };
        let phytium1500a = if phytium1500a_result.is_success() {
            let record = parse_phytium1500a_info(&phytium1500a_result.stdout);
            (!record.is_empty()).then_some(record)
        } else {
            None
        };
        let sensors_temperatures = if sensors_result.is_success() {
            parse_sensors_cpu_temperatures(&sensors_result.stdout)
        } else {
            HashMap::new()
        };
        let lscpu_contributed = lscpu.is_some();
        let lshw_contributed = lshw.is_some();
        let dmi_contributed = dmi.iter().any(hw_parser::DmidecodeCpuRecord::is_useful);
        let proc_cpuinfo_contributed = proc_cpuinfo.is_some();
        let proc_hardware_contributed = proc_hardware.is_some();
        let cpu_sysfs_contributed = cpu_sysfs.is_some();
        let cpufreq_contributed = cpufreq.is_some();
        let sensors_contributed = !sensors_temperatures.is_empty();
        let phytium1500a_contributed = phytium1500a.is_some();
        if lscpu.is_none()
            && lshw.is_none()
            && !dmi_contributed
            && proc_cpuinfo.is_none()
            && proc_hardware.is_none()
            && cpu_sysfs.is_none()
            && cpufreq.is_none()
            && !phytium1500a_contributed
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
                merge_cpu_record_fallback(
                    merge_cpu_record_fallback(
                        merge_cpu_record_fallback(lscpu, proc_cpuinfo),
                        proc_hardware,
                    ),
                    cpu_sysfs,
                ),
                phytium1500a,
            ),
            lshw,
            &dmi,
        );
        if let Some(cpufreq) = cpufreq.as_ref() {
            merged.max_freq_mhz = merged.max_freq_mhz.or(cpufreq.record.cpu_max_mhz);
            merged.min_freq_mhz = merged.min_freq_mhz.or(cpufreq.record.cpu_min_mhz);
            merged.current_freq_mhz = merged.current_freq_mhz.or(cpufreq.record.cpu_mhz);
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
        let extensions = cpu_extensions_from_flags(&merged.flags);
        let cache_entries = read_cpu_cache_entries(ctx, merged.threads).await;
        let logical_cpus = read_logical_cpus(ctx, &sensors_temperatures).await;
        let hw_platform = detect_hw_platform(ctx, merged.name.as_deref()).await;
        let (frequency_display, frequency_is_range) = hw_parser::format_cpu_frequency_display(
            merged.max_freq_mhz,
            merged.min_freq_mhz,
            merged.current_freq_mhz,
        );
        let overview = merged
            .name
            .as_deref()
            .and_then(|name| hw_parser::format_cpu_overview(name, merged.cores, merged.threads));
        let display_name = merged.name.clone().unwrap_or_else(|| "CPU".to_string());
        let mut device = Device::new(
            "cpu:0",
            DeviceKind::Cpu,
            display_name,
            DeviceProperties::Cpu(Box::new(CpuInfo {
                name: merged.name,
                vendor,
                architecture,
                cores: merged.cores,
                enabled_cores: merged.enabled_cores,
                threads: merged.threads,
                online_threads: merged.online_threads,
                online_cores: merged.online_cores,
                threads_per_core: merged.threads_per_core,
                sockets: merged.sockets,
                socket_designations: merged.socket_designations,
                serial_numbers: merged.serial_numbers,
                max_freq_mhz: merged.max_freq_mhz,
                min_freq_mhz: merged.min_freq_mhz,
                current_freq_mhz: merged.current_freq_mhz,
                external_clock_mhz: merged.external_clock_mhz,
                frequency_display,
                frequency_is_range,
                overview,
                family: merged.family,
                cpu_implementer: merged.cpu_implementer,
                cpu_architecture: merged.cpu_architecture,
                cpu_variant: merged.cpu_variant,
                cpu_part: merged.cpu_part,
                cpu_revision: merged.cpu_revision,
                model: merged.model,
                stepping: merged.stepping,
                bogomips: merged.bogomips,
                virtualization: merged.virtualization,
                l1d_cache: merged.l1d_cache,
                l1i_cache: merged.l1i_cache,
                l2_cache: merged.l2_cache,
                l3_cache: merged.l3_cache,
                l4_cache: merged.l4_cache,
                caches: cache_entries,
                clflush_size_bytes: merged.clflush_size_bytes,
                flags: merged.flags,
                extensions,
                logical_cpus,
                hw_platform,
                scaling_governor: cpufreq
                    .as_ref()
                    .and_then(|cpufreq| cpufreq.scaling_governor.clone()),
                scaling_available_governors: cpufreq
                    .as_ref()
                    .map(|cpufreq| cpufreq.scaling_available_governors.clone())
                    .unwrap_or_default(),
                scaling_available_frequencies_khz: cpufreq
                    .as_ref()
                    .map(|cpufreq| cpufreq.scaling_available_frequencies_khz.clone())
                    .unwrap_or_default(),
                scaling_setspeed_supported: cpufreq
                    .as_ref()
                    .is_some_and(|cpufreq| cpufreq.scaling_setspeed_supported),
            })),
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
        if cpu_sysfs_contributed {
            device = device.with_source(SourceEvidence {
                source: "/sys/devices/system/cpu".to_string(),
                kind: SourceKind::Sysfs,
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
        if sensors_contributed {
            device = device.with_source(SourceEvidence {
                source: sensors_result.source,
                kind: SourceKind::Command,
                status: SourceStatus::Success,
                summary: None,
            });
        }
        if phytium1500a_contributed {
            device = device.with_source(SourceEvidence {
                source: "/sys/phytium1500a_info".to_string(),
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
    primary.online_threads = primary.online_threads.or(fallback.online_threads);
    primary.online_cores = primary.online_cores.or(fallback.online_cores);
    primary.threads_per_core = primary.threads_per_core.or(fallback.threads_per_core);
    primary.model_name = primary.model_name.or(fallback.model_name);
    primary.vendor = primary.vendor.or(fallback.vendor);
    primary.cores_per_socket = primary.cores_per_socket.or(fallback.cores_per_socket);
    primary.sockets = primary.sockets.or(fallback.sockets);
    primary.cpu_mhz = primary.cpu_mhz.or(fallback.cpu_mhz);
    primary.cpu_max_mhz = primary.cpu_max_mhz.or(fallback.cpu_max_mhz);
    primary.cpu_min_mhz = primary.cpu_min_mhz.or(fallback.cpu_min_mhz);
    primary.cpu_family = primary.cpu_family.or(fallback.cpu_family);
    primary.cpu_implementer = primary.cpu_implementer.or(fallback.cpu_implementer);
    primary.cpu_architecture = primary.cpu_architecture.or(fallback.cpu_architecture);
    primary.cpu_variant = primary.cpu_variant.or(fallback.cpu_variant);
    primary.cpu_part = primary.cpu_part.or(fallback.cpu_part);
    primary.cpu_revision = primary.cpu_revision.or(fallback.cpu_revision);
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
    primary.l4_cache = primary.l4_cache.or(fallback.l4_cache);
    primary.clflush_size_bytes = primary.clflush_size_bytes.or(fallback.clflush_size_bytes);

    Some(primary)
}

async fn read_cpu_sysfs(ctx: &ProbeContext<'_>) -> Option<hw_parser::CpuRecord> {
    let mut cpu_paths = ctx
        .runner
        .glob("/sys/devices/system/cpu/cpu*")
        .await
        .paths
        .into_iter()
        .filter(|path| cpu_index_from_sysfs_path(path).is_some())
        .collect::<Vec<_>>();
    cpu_paths.sort_by_key(|path| cpu_index_from_sysfs_path(path).unwrap_or(u32::MAX));

    let mut record = hw_parser::CpuRecord::default();
    if !cpu_paths.is_empty() {
        record.threads = u32::try_from(cpu_paths.len()).ok();
    }
    let online_cpu_indices =
        read_optional_trimmed(ctx, Path::new("/sys/devices/system/cpu/online"))
            .await
            .and_then(|value| parse_cpu_list_indices(&value));
    if let Some(indices) = online_cpu_indices.as_ref() {
        record.online_threads = u32::try_from(indices.len()).ok();
    }

    let mut packages = Vec::new();
    let mut cores = Vec::new();
    let mut online_cores = Vec::new();
    let mut sibling_counts = Vec::new();
    for path in &cpu_paths {
        let cpu_index = cpu_index_from_sysfs_path(path);
        let package = read_sysfs_cpu_package(ctx, path).await;
        if let Some(package) = package.as_deref() {
            push_unique_string(&mut packages, package);
        }

        let core_key = read_optional_trimmed(ctx, &path.join("topology/core_id"))
            .await
            .filter(|value| !value.trim().is_empty());
        let siblings =
            read_optional_trimmed(ctx, &path.join("topology/thread_siblings_list")).await;
        if let Some(count) = siblings.as_deref().and_then(parse_cpu_list_count) {
            sibling_counts.push(count);
        }
        let core_key = core_key.or_else(|| siblings.clone());
        if let Some(core_key) = core_key {
            let package = package.unwrap_or_else(|| "0".to_string());
            let core_key = format!("{package}:{core_key}");
            push_unique_string(&mut cores, &core_key);
            if online_cpu_indices
                .as_ref()
                .zip(cpu_index)
                .is_some_and(|(indices, cpu_index)| indices.contains(&cpu_index))
            {
                push_unique_string(&mut online_cores, &core_key);
            }
        }
    }

    if !packages.is_empty() {
        record.sockets = u32::try_from(packages.len()).ok();
    } else if !cores.is_empty() {
        record.sockets = Some(1);
    }
    if let (Some(sockets), false) = (record.sockets, cores.is_empty()) {
        let core_count = u32::try_from(cores.len()).ok();
        record.cores_per_socket = core_count
            .filter(|cores| sockets > 0 && cores % sockets == 0)
            .map(|cores| cores / sockets);
    }
    record.threads_per_core = uniform_nonzero(&sibling_counts).or_else(|| {
        record
            .threads
            .zip(u32::try_from(cores.len()).ok())
            .and_then(|(threads, cores)| {
                (cores > 0 && threads % cores == 0).then(|| threads / cores)
            })
            .filter(|value| *value > 0)
    });
    record.online_cores = u32::try_from(online_cores.len())
        .ok()
        .filter(|value| *value > 0)
        .or_else(|| infer_online_cores(record.online_threads, record.threads_per_core));

    if let Some(path) = cpu_paths.first() {
        let cache = read_cpu_sysfs_cache(ctx, path, record.threads).await;
        record.l1d_cache = cache.l1d_cache;
        record.l1i_cache = cache.l1i_cache;
        record.l2_cache = cache.l2_cache;
        record.l3_cache = cache.l3_cache;
        record.l4_cache = cache.l4_cache;
    }

    (!record.is_empty()).then_some(record)
}

async fn read_sysfs_cpu_package(ctx: &ProbeContext<'_>, cpu_path: &Path) -> Option<String> {
    let value = read_optional_trimmed(ctx, &cpu_path.join("topology/physical_package_id")).await?;
    match value.parse::<i32>() {
        Ok(value) if value < 0 => Some("0".to_string()),
        _ => Some(value),
    }
}

/// Deepin-style per-cache-index enumeration for `CpuInfo.caches`. Walks
/// `/sys/devices/system/cpu/cpu0/cache/index*` and returns one entry per
/// index, matching Deepin's `DeviceCpu` structured cache table rather
/// than the four aggregated L1d/L1i/L2/L3 strings.
async fn read_cpu_cache_entries(
    ctx: &ProbeContext<'_>,
    threads: Option<u32>,
) -> Vec<hw_model::CpuCacheEntry> {
    let cpu0 = Path::new("/sys/devices/system/cpu/cpu0");
    let pattern = format!("{}/cache/index*", cpu0.display());
    let mut paths = ctx.runner.glob(&pattern).await.paths;
    paths.sort();

    let mut entries = Vec::new();
    for path in paths {
        let level = read_optional_trimmed(ctx, &path.join("level"))
            .await
            .and_then(|value| value.parse::<u32>().ok());
        let Some(level) = level else {
            continue;
        };
        let kind = read_optional_trimmed(ctx, &path.join("type")).await;
        let raw_size = read_optional_trimmed(ctx, &path.join("size")).await;
        let ways = read_optional_trimmed(ctx, &path.join("ways_of_associativity"))
            .await
            .and_then(|value| value.parse::<u32>().ok());
        let line_size = read_optional_trimmed(ctx, &path.join("coherency_line_size"))
            .await
            .and_then(|value| value.parse::<u32>().ok());
        let sets = read_optional_trimmed(ctx, &path.join("number_of_sets"))
            .await
            .and_then(|value| value.parse::<u32>().ok());
        let shared_cpu_list = read_optional_trimmed(ctx, &path.join("shared_cpu_list")).await;
        let shared_cpu_count = shared_cpu_list.as_deref().and_then(parse_cpu_list_count);

        let size_bytes = raw_size.as_deref().and_then(parse_sysfs_cache_bytes);
        let size = raw_size.as_ref().and_then(|raw| {
            total_sysfs_cache(raw, shared_cpu_list.as_deref(), threads)
                .or_else(|| Some(raw.clone()))
        });

        entries.push(hw_model::CpuCacheEntry {
            level,
            kind,
            size,
            size_bytes,
            ways_of_associativity: ways,
            coherency_line_size: line_size,
            number_of_sets: sets,
            shared_cpu_list,
            shared_cpu_count,
        });
    }
    entries
}

/// Deepin `LoadCpuInfoThread` per-logical-CPU detail: enumerates
/// `/sys/devices/system/cpu/cpu*` and returns one `LogicalCpuInfo` per
/// online logical processor, carrying package/core ids and per-core
/// current/min/max cpufreq (kHz to MHz) plus BogoMIPS from `/proc/cpuinfo`.
async fn read_logical_cpus(
    ctx: &ProbeContext<'_>,
    temperatures_by_core: &HashMap<u32, f32>,
) -> Vec<hw_model::LogicalCpuInfo> {
    let mut paths = ctx
        .runner
        .glob("/sys/devices/system/cpu/cpu*")
        .await
        .paths
        .into_iter()
        .filter(|path| cpu_index_from_sysfs_path(path).is_some())
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| cpu_index_from_sysfs_path(path).unwrap_or(u32::MAX));

    let online_indices = read_optional_trimmed(ctx, Path::new("/sys/devices/system/cpu/online"))
        .await
        .and_then(|value| parse_cpu_list_indices(&value));

    let bogomips_by_cpu = read_bogomips_by_cpu(ctx).await;

    let mut logical = Vec::new();
    for path in paths {
        let Some(processor) = cpu_index_from_sysfs_path(&path) else {
            continue;
        };
        let physical_id = read_optional_trimmed(ctx, &path.join("topology/physical_package_id"))
            .await
            .and_then(|value| value.parse::<i32>().ok())
            .and_then(|value| u32::try_from(value).ok());
        let core_id = read_optional_trimmed(ctx, &path.join("topology/core_id"))
            .await
            .and_then(|value| value.parse::<i32>().ok())
            .and_then(|value| u32::try_from(value).ok());
        let cur = read_cpufreq_mhz(ctx, &path.join("cpufreq/scaling_cur_freq")).await;
        let min = read_cpufreq_mhz(ctx, &path.join("cpufreq/cpuinfo_min_freq"))
            .await
            .or(read_cpufreq_mhz(ctx, &path.join("cpufreq/scaling_min_freq")).await);
        let max = read_cpufreq_mhz(ctx, &path.join("cpufreq/cpuinfo_max_freq"))
            .await
            .or(read_cpufreq_mhz(ctx, &path.join("cpufreq/scaling_max_freq")).await);
        let online = online_indices
            .as_ref()
            .map(|indices| indices.contains(&processor))
            .unwrap_or(true);
        let bogomips = bogomips_by_cpu.get(&processor).cloned();
        let temperature_celsius = core_id
            .and_then(|core_id| temperatures_by_core.get(&core_id).copied())
            .or_else(|| temperatures_by_core.get(&processor).copied());
        logical.push(hw_model::LogicalCpuInfo {
            processor,
            physical_id,
            core_id,
            current_freq_mhz: cur,
            min_freq_mhz: min,
            max_freq_mhz: max,
            bogomips,
            online,
            temperature_celsius,
        });
    }
    logical
}

async fn read_bogomips_by_cpu(ctx: &ProbeContext<'_>) -> HashMap<u32, String> {
    let mut out = HashMap::new();
    let result = ctx.runner.read_file(Path::new("/proc/cpuinfo")).await;
    if !result.is_success() {
        return out;
    }
    let mut current_processor: Option<u32> = None;
    for line in result.stdout.lines() {
        let Some((key, value)) = line.split_once(':') else {
            if line.trim().is_empty() {
                current_processor = None;
            }
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key == "processor" {
            current_processor = value.parse::<u32>().ok();
        } else if key.eq_ignore_ascii_case("BogoMIPS") || key.eq_ignore_ascii_case("bogomips") {
            if let Some(cpu) = current_processor {
                if !value.is_empty() {
                    out.entry(cpu).or_insert_with(|| value.to_string());
                }
            }
        }
    }
    out
}

/// Deepin `Common::isHwPlatform()` runtime signal. Combines the CPU name,
/// DMI system manufacturer, and `/proc/hardware` string. When any of them
/// carries a HW-classified marker (Kunpeng/Kirin/HW990/PGUW/KLVV),
/// `CpuInfo.hw_platform = true`; the UI can source the CPU name from
/// DMI Version so Overview reads the customer branding.
async fn detect_hw_platform(ctx: &ProbeContext<'_>, name: Option<&str>) -> bool {
    if hw_parser::is_hw_platform_marker(None, name) {
        return true;
    }
    let sys_vendor = read_sysfs_dmi_value(ctx, "sys_vendor").await;
    let product_name = read_sysfs_dmi_value(ctx, "product_name").await;
    if hw_parser::is_hw_platform_marker(sys_vendor.as_deref(), product_name.as_deref()) {
        return true;
    }
    let hw_result = ctx.runner.read_file(Path::new("/proc/hardware")).await;
    if hw_result.is_success()
        && hw_parser::is_hw_platform_marker(None, Some(hw_result.stdout.as_str()))
    {
        return true;
    }
    false
}

async fn read_cpu_sysfs_cache(
    ctx: &ProbeContext<'_>,
    cpu_path: &Path,
    threads: Option<u32>,
) -> hw_parser::CpuRecord {
    let pattern = format!("{}/cache/index*", cpu_path.display());
    let mut record = hw_parser::CpuRecord::default();
    for path in ctx.runner.glob(&pattern).await.paths {
        let level = read_optional_trimmed(ctx, &path.join("level"))
            .await
            .and_then(|value| value.parse::<u32>().ok());
        let cache_type = read_optional_trimmed(ctx, &path.join("type"))
            .await
            .map(|value| value.to_ascii_lowercase());
        let Some(size) = read_optional_trimmed(ctx, &path.join("size")).await else {
            continue;
        };
        let shared_cpu_list = read_optional_trimmed(ctx, &path.join("shared_cpu_list")).await;
        let cache = total_sysfs_cache(&size, shared_cpu_list.as_deref(), threads)
            .unwrap_or_else(|| size.clone());

        match (level, cache_type.as_deref()) {
            (Some(1), Some("data")) => record.l1d_cache = record.l1d_cache.or(Some(cache)),
            (Some(1), Some("instruction")) => record.l1i_cache = record.l1i_cache.or(Some(cache)),
            (Some(2), _) => record.l2_cache = record.l2_cache.or(Some(cache)),
            (Some(3), _) => record.l3_cache = record.l3_cache.or(Some(cache)),
            (Some(4), _) => record.l4_cache = record.l4_cache.or(Some(cache)),
            _ => {}
        }
    }
    record
}

fn cpu_index_from_sysfs_path(path: &Path) -> Option<u32> {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("cpu"))
        .filter(|suffix| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
        .and_then(|suffix| suffix.parse().ok())
}

fn push_unique_string(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|seen| seen == value) {
        values.push(value.to_string());
    }
}

fn uniform_nonzero(values: &[u32]) -> Option<u32> {
    let first = *values.first()?;
    (first > 0 && values.iter().all(|value| *value == first)).then_some(first)
}

fn total_sysfs_cache(
    size: &str,
    shared_cpu_list: Option<&str>,
    threads: Option<u32>,
) -> Option<String> {
    let bytes = parse_sysfs_cache_bytes(size)?;
    let groups = shared_cpu_list
        .and_then(parse_cpu_list_count)
        .and_then(|shared| threads?.checked_div(shared))
        .filter(|groups| *groups > 0)
        .unwrap_or(1);
    format_cache_bytes(bytes.checked_mul(u64::from(groups))?)
}

fn parse_cpu_list_count(value: &str) -> Option<u32> {
    let mut count = 0u32;
    for part in value.trim().split(',').map(str::trim) {
        if part.is_empty() {
            continue;
        }
        let add = if let Some((start, end)) = part.split_once('-') {
            let start = start.trim().parse::<u32>().ok()?;
            let end = end.trim().parse::<u32>().ok()?;
            end.checked_sub(start)?.checked_add(1)?
        } else {
            part.parse::<u32>().ok()?;
            1
        };
        count = count.checked_add(add)?;
    }
    (count > 0).then_some(count)
}

fn parse_cpu_list_indices(value: &str) -> Option<Vec<u32>> {
    let mut indices = Vec::new();
    for part in value.trim().split(',').map(str::trim) {
        if part.is_empty() {
            continue;
        }
        if let Some((start, end)) = part.split_once('-') {
            let start = start.trim().parse::<u32>().ok()?;
            let end = end.trim().parse::<u32>().ok()?;
            if end < start {
                return None;
            }
            for index in start..=end {
                if !indices.contains(&index) {
                    indices.push(index);
                }
            }
        } else {
            let index = part.parse::<u32>().ok()?;
            if !indices.contains(&index) {
                indices.push(index);
            }
        }
    }
    (!indices.is_empty()).then_some(indices)
}

fn infer_online_cores(online_threads: Option<u32>, threads_per_core: Option<u32>) -> Option<u32> {
    let online_threads = online_threads?;
    let threads_per_core = threads_per_core?.max(1);
    online_threads
        .checked_add(threads_per_core.checked_sub(1)?)?
        .checked_div(threads_per_core)
        .filter(|value| *value > 0)
}

fn parse_sysfs_cache_bytes(value: &str) -> Option<u64> {
    let value = value.trim();
    let split = value
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .unwrap_or(value.len());
    let number = value.get(..split)?.parse::<f64>().ok()?;
    let unit = value.get(split..)?.trim().to_ascii_lowercase();
    let multiplier = match unit.as_str() {
        "" | "b" => 1.0,
        "k" | "kb" | "kib" => 1024.0,
        "m" | "mb" | "mib" => 1024.0 * 1024.0,
        "g" | "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((number * multiplier).round() as u64)
}

fn format_cache_bytes(bytes: u64) -> Option<String> {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    if bytes >= GIB && bytes % GIB == 0 {
        Some(format!("{} GiB", bytes / GIB))
    } else if bytes >= MIB && bytes % MIB == 0 {
        Some(format!("{} MiB", bytes / MIB))
    } else if bytes >= KIB && bytes % KIB == 0 {
        Some(format!("{} KiB", bytes / KIB))
    } else if bytes > 0 {
        Some(format!("{bytes} B"))
    } else {
        None
    }
}

#[derive(Default)]
struct CpuCpufreqInfo {
    record: hw_parser::CpuRecord,
    scaling_governor: Option<String>,
    scaling_available_governors: Vec<String>,
    scaling_available_frequencies_khz: Vec<u32>,
    scaling_setspeed_supported: bool,
}

impl CpuCpufreqInfo {
    fn is_empty(&self) -> bool {
        self.record.is_empty()
            && self.scaling_governor.is_none()
            && self.scaling_available_governors.is_empty()
            && self.scaling_available_frequencies_khz.is_empty()
            && !self.scaling_setspeed_supported
    }
}

async fn read_cpu_cpufreq(ctx: &ProbeContext<'_>) -> Option<CpuCpufreqInfo> {
    let base = Path::new("/sys/devices/system/cpu/cpu0/cpufreq");
    let mut cpu_max_mhz = read_cpufreq_mhz(ctx, &base.join("cpuinfo_max_freq")).await;
    if cpu_max_mhz.is_none() {
        cpu_max_mhz = read_cpufreq_mhz(ctx, &base.join("scaling_max_freq")).await;
    }
    let mut cpu_min_mhz = read_cpufreq_mhz(ctx, &base.join("cpuinfo_min_freq")).await;
    if cpu_min_mhz.is_none() {
        cpu_min_mhz = read_cpufreq_mhz(ctx, &base.join("scaling_min_freq")).await;
    }
    let mut cpu_mhz = read_average_scaling_cur_freq(ctx).await;
    if cpu_mhz.is_none() {
        cpu_mhz = read_cpufreq_mhz(ctx, &base.join("scaling_cur_freq")).await;
    }
    let scaling_setspeed = read_optional_trimmed(ctx, &base.join("scaling_setspeed")).await;
    if cpu_mhz.is_none() {
        cpu_mhz = scaling_setspeed.as_deref().and_then(parse_cpufreq_khz);
    }
    let info = CpuCpufreqInfo {
        record: hw_parser::CpuRecord {
            cpu_mhz,
            cpu_max_mhz,
            cpu_min_mhz,
            ..Default::default()
        },
        scaling_governor: read_optional_trimmed(ctx, &base.join("scaling_governor")).await,
        scaling_available_governors: read_optional_trimmed(
            ctx,
            &base.join("scaling_available_governors"),
        )
        .await
        .map(|value| {
            value
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default(),
        scaling_available_frequencies_khz: read_optional_trimmed(
            ctx,
            &base.join("scaling_available_frequencies"),
        )
        .await
        .map(|value| parse_cpufreq_khz_list(&value))
        .unwrap_or_default(),
        scaling_setspeed_supported: scaling_setspeed
            .as_deref()
            .is_some_and(cpufreq_setspeed_supported),
    };

    (!info.is_empty()).then_some(info)
}

async fn read_average_scaling_cur_freq(ctx: &ProbeContext<'_>) -> Option<u32> {
    let glob = ctx
        .runner
        .glob("/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq")
        .await;
    let mut total_khz = 0u64;
    let mut count = 0u64;
    for path in glob.paths {
        let result = ctx.runner.read_file(&path).await;
        if let Some(khz) = result
            .is_success()
            .then(|| result.stdout.trim().parse::<u64>().ok())
            .flatten()
            .filter(|value| *value > 0)
        {
            total_khz = total_khz.checked_add(khz)?;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let mhz = (total_khz / count + 500) / 1000;
    u32::try_from(mhz).ok().filter(|value| *value > 0)
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

fn parse_cpufreq_khz_list(value: &str) -> Vec<u32> {
    value
        .split_whitespace()
        .filter_map(|part| part.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .collect()
}

fn cpufreq_setspeed_supported(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && !value.eq_ignore_ascii_case("<unsupported>")
}

fn parse_sensors_cpu_temperatures(input: &str) -> HashMap<u32, f32> {
    let mut temperatures = HashMap::new();
    for line in input.lines() {
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };
        let Some(index) = sensors_cpu_index(label) else {
            continue;
        };
        if let Some(temperature) = parse_temperature_celsius(value) {
            temperatures.entry(index).or_insert(temperature);
        }
    }
    temperatures
}

fn sensors_cpu_index(label: &str) -> Option<u32> {
    let label = label.trim().to_ascii_lowercase();
    let suffix = label
        .strip_prefix("core")
        .or_else(|| label.strip_prefix("cpu"))?;
    let digits = suffix
        .trim_start_matches(|ch: char| ch.is_ascii_whitespace() || ch == '-' || ch == '_')
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

fn parse_temperature_celsius(value: &str) -> Option<f32> {
    let start = value.find(|ch: char| ch == '+' || ch == '-' || ch.is_ascii_digit())?;
    let tail = &value[start..];
    let end = tail
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '+' || ch == '-'))
        .unwrap_or(tail.len());
    tail[..end].parse::<f32>().ok()
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
        let mut serial = read_optional_trimmed(ctx, &path.join("device/serial")).await;
        if serial.is_none() {
            serial = storage_serial_fallback(ctx, &path).await;
        }
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
        let rotational = read_optional_trimmed(ctx, &path.join("queue/rotational")).await;
        let spec_version = read_optional_trimmed(ctx, &path.join("device/spec_version")).await;
        let modalias = read_optional_trimmed(ctx, &path.join("device/modalias")).await;
        let uevent = read_optional_trimmed(ctx, &path.join("device/uevent")).await;
        let vid_pid = uevent
            .as_deref()
            .and_then(|uevent| parse_uevent_value(uevent, "PRODUCT"));
        let media_type = rotational
            .as_deref()
            .and_then(storage_media_type_from_rotational);
        let interface = if spec_version.is_some() {
            Some("ufs".to_string())
        } else {
            modalias
                .as_deref()
                .and_then(storage_interface_from_modalias)
        };
        let rotation_rate = rotational
            .as_deref()
            .and_then(storage_rotation_rate_from_rotational);

        let mut device = Device::new(
            device_id::storage(None, serial.as_deref(), &node),
            DeviceKind::Storage,
            model.clone().unwrap_or_else(|| node.clone()),
            DeviceProperties::Storage(StorageInfo {
                device_node: Some(node),
                size_bytes,
                size_display: storage_size_display(size_bytes, interface.as_deref()),
                media_type,
                interface,
                firmware,
                wwn,
                rotation_rate,
                ufs_spec_version: spec_version,
                vid_pid: vid_pid.clone(),
                phys_id: vid_pid,
                modalias,
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
        devices.push(apply_storage_parent_ref(apply_storage_vendor_fallback(
            apply_storage_smartctl(ctx, device).await,
        )));
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
    apply_storage_vendor_fallback(device).with_source(SourceEvidence {
        source: source.to_string(),
        kind: SourceKind::Command,
        status: SourceStatus::Success,
        summary: None,
    })
}

async fn apply_storage_sysfs_enrichment(
    ctx: &ProbeContext<'_>,
    mut device: Device,
    name: &str,
) -> Device {
    let sysfs_path = Path::new("/sys/block").join(name);
    let spec_version = read_optional_trimmed(ctx, &sysfs_path.join("device/spec_version")).await;
    let rotational = read_optional_trimmed(ctx, &sysfs_path.join("queue/rotational")).await;
    let modalias = read_optional_trimmed(ctx, &sysfs_path.join("device/modalias")).await;
    let uevent = read_optional_trimmed(ctx, &sysfs_path.join("device/uevent")).await;
    let vid_pid = uevent
        .as_deref()
        .and_then(|uevent| parse_uevent_value(uevent, "PRODUCT"));
    let serial_fallback = if device.serial.is_none() {
        storage_serial_fallback(ctx, &sysfs_path).await
    } else {
        None
    };
    let mut contributed = false;

    if let Some(serial) = serial_fallback {
        device.serial = Some(serial);
        if let DeviceProperties::Storage(storage) = &device.properties {
            if let Some(node) = storage.device_node.as_deref() {
                device.id =
                    device_id::storage(storage.wwn.as_deref(), device.serial.as_deref(), node);
            }
        }
        contributed = true;
    }

    if let DeviceProperties::Storage(storage) = &mut device.properties {
        if spec_version.is_some() {
            if storage.interface.as_deref() != Some("ufs") {
                storage.interface = Some("ufs".to_string());
                contributed = true;
            }
            if storage.ufs_spec_version.is_none() {
                storage.ufs_spec_version = spec_version;
                contributed = true;
            }
        }
        if let Some(rotational) = rotational.as_deref() {
            if storage.media_type.is_none() {
                storage.media_type = storage_media_type_from_rotational(rotational);
                contributed |= storage.media_type.is_some();
            }
            if storage.rotation_rate.is_none() {
                storage.rotation_rate = storage_rotation_rate_from_rotational(rotational);
                contributed |= storage.rotation_rate.is_some();
            }
        }
        if let Some(modalias_ref) = modalias.as_deref() {
            if storage.interface.is_none() {
                storage.interface = storage_interface_from_modalias(modalias_ref);
                contributed |= storage.interface.is_some();
            }
        }
        if storage.modalias.is_none() && modalias.is_some() {
            storage.modalias = modalias;
            contributed = true;
        }
        if storage.vid_pid.is_none() && vid_pid.is_some() {
            storage.vid_pid = vid_pid.clone();
            storage.phys_id = storage.phys_id.take().or(vid_pid);
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
    device
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
    let Some(interface) = storage_interface_for_pci_controller(&device) else {
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
                .is_some_and(|class| storage_controller_class_matches_interface(interface, class))
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

fn storage_interface_for_pci_controller(device: &Device) -> Option<&str> {
    let DeviceProperties::Storage(storage) = &device.properties else {
        return None;
    };
    storage
        .interface
        .as_deref()
        .filter(|interface| matches!(*interface, "sata" | "ata" | "scsi"))
}

fn storage_controller_class_matches_interface(interface: &str, class: &str) -> bool {
    let class = class.trim_start_matches("0x").trim_start_matches("0X");
    match interface {
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
        if storage.speed.is_none() && record.speed.is_some() {
            storage.speed = record.speed.clone();
            contributed = true;
        }
        if storage.capabilities.is_empty() && !record.capabilities.is_empty() {
            storage.capabilities = record.capabilities.clone();
            device.capabilities.extend(record.capabilities.clone());
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
    let interface = match &device.properties {
        DeviceProperties::Storage(storage) => storage.interface.clone(),
        _ => None,
    };

    let result = run_storage_smartctl(ctx, &node, interface.as_deref()).await;
    let source_status = if result.is_success() {
        SourceStatus::Success
    } else {
        SourceStatus::Failed
    };
    if !result.is_success() && result.stdout.trim().is_empty() {
        return device;
    }

    let Ok(smart) = parse_smartctl_json(&result.stdout) else {
        device.warnings.push(
            ScanWarning::new(
                "parse_failed",
                format!("storage source '{}' could not be parsed", result.source),
            )
            .with_source(result.source),
        );
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

async fn run_storage_smartctl(
    ctx: &ProbeContext<'_>,
    node: &str,
    interface: Option<&str>,
) -> SourceResult {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new(
                "smartctl",
                ["-a".to_string(), "-j".to_string(), node.to_string()],
            ),
            ctx.timeout,
        )
        .await;
    if !should_retry_storage_smartctl_with_sat(&result, interface) {
        return result;
    }
    ctx.runner
        .run_command(
            &CommandSpec::new(
                "smartctl",
                [
                    "-a".to_string(),
                    "-j".to_string(),
                    "-d".to_string(),
                    "sat".to_string(),
                    node.to_string(),
                ],
            ),
            ctx.timeout,
        )
        .await
}

fn should_retry_storage_smartctl_with_sat(result: &SourceResult, interface: Option<&str>) -> bool {
    !result.is_success()
        && interface.is_some_and(|value| value.eq_ignore_ascii_case("usb"))
        && result.stdout.contains("Read Device Identity failed:")
}

fn is_ignored_block_device(name: &str) -> bool {
    ["loop", "ram", "zram", "dm-", "md", "sr"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

fn storage_media_type_from_rotational(rotational: &str) -> Option<String> {
    match rotational {
        "0" => Some("ssd".to_string()),
        "1" => Some("hdd".to_string()),
        _ => None,
    }
}

fn storage_rotation_rate_from_rotational(rotational: &str) -> Option<String> {
    match rotational {
        "0" => Some("Solid State Device".to_string()),
        "1" => Some("Rotating Media".to_string()),
        _ => None,
    }
}

fn normalize_storage_interface(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn storage_interface_from_modalias(modalias: &str) -> Option<String> {
    let prefix = modalias.split(':').next().unwrap_or(modalias);
    match prefix.to_ascii_lowercase().as_str() {
        "nvme" => Some("nvme".to_string()),
        "pci" if modalias.contains("cc0108") || modalias.contains("cc010802") => {
            Some("nvme".to_string())
        }
        "usb" => Some("usb".to_string()),
        "scsi" => Some("scsi".to_string()),
        "sdio" | "mmc" => Some("mmc".to_string()),
        _ => None,
    }
}

fn storage_size_display(size_bytes: Option<u64>, interface: Option<&str>) -> Option<String> {
    let size = size_bytes?;
    let gb = 1_000_000_000_u64;
    let is_usb = interface.is_some_and(|value| value.eq_ignore_ascii_case("usb"));
    let display = if size > 255 * gb && size < 257 * gb {
        "256 GB"
    } else if size > 511 * gb && size < 513 * gb {
        "512 GB"
    } else if size > 999 * gb && size < 1025 * gb {
        "1 TB"
    } else if size > 1999 * gb && size < 2049 * gb {
        "2 TB"
    } else if is_usb && size > 15 * gb && size < 17 * gb {
        "16 GB"
    } else if is_usb && size > 31 * gb && size < 33 * gb {
        "32 GB"
    } else if is_usb && size > 63 * gb && size < 65 * gb {
        "64 GB"
    } else if is_usb && size > 127 * gb && size < 129 * gb {
        "128 GB"
    } else {
        return None;
    };
    Some(display.to_string())
}

async fn storage_serial_fallback(ctx: &ProbeContext<'_>, sysfs_path: &Path) -> Option<String> {
    let device_path = sysfs_path.join("device");
    let device_name = read_optional_trimmed(ctx, &device_path.join("name")).await;
    let bootdevice_name = read_optional_trimmed(ctx, Path::new("/proc/bootdevice/name")).await;
    if device_name.is_some() && device_name == bootdevice_name {
        if let Some(cid) = read_optional_trimmed(ctx, Path::new("/proc/bootdevice/cid")).await {
            return Some(cid);
        }
    }
    read_optional_trimmed(ctx, &device_path.join("unique_number")).await
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
            fallback.consumed = storage_consumed_refs(&fallback.devices);
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
            let sysfs_path = Path::new("/sys/block").join(&name);
            let mut serial = dev.serial;
            if serial.is_none() {
                serial = storage_serial_fallback(ctx, &sysfs_path).await;
            }
            let interface = dev.tran.as_deref().map(normalize_storage_interface);
            let mut device = Device::new(
                device_id::storage(wwn.as_deref(), serial.as_deref(), &node),
                DeviceKind::Storage,
                dev.model.clone().unwrap_or_else(|| node.clone()),
                DeviceProperties::Storage(StorageInfo {
                    device_node: Some(node),
                    size_bytes: dev.size,
                    size_display: storage_size_display(dev.size, interface.as_deref()),
                    interface,
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
            device.serial = serial;
            let device = apply_storage_sysfs_enrichment(ctx, device, &name).await;
            let device = apply_storage_driver(ctx, device, &name).await;
            let device = apply_storage_lshw_storage_enrichment(device, &lshw_storage);
            let device = apply_storage_lspci_enrichment(device, &lspci);
            let device = apply_storage_lshw_enrichment(device, &lshw);
            let device = apply_storage_hwinfo_enrichment(device, &hwinfo);
            let device = apply_storage_hdparm_enrichment(ctx, device).await;
            devices.push(apply_storage_parent_ref(apply_storage_vendor_fallback(
                apply_storage_smartctl(ctx, device).await,
            )));
        }
        if devices.is_empty() {
            let fallback_devices = storage_devices_from_sysfs(ctx).await;
            let consumed = storage_consumed_refs(&fallback_devices);
            return ProbeResult {
                devices: fallback_devices,
                warnings: vec![ScanWarning::new(
                    "source_empty",
                    "storage source produced no disk records",
                )
                .with_source(result.source)],
                consumed,
            };
        }
        ProbeResult {
            consumed: storage_consumed_refs(&devices),
            devices,
            warnings: Vec::new(),
        }
    }
}

fn apply_storage_parent_ref(mut device: Device) -> Device {
    let Some(BusInfo::Pci { address, .. }) = device.bus.as_ref() else {
        return device;
    };
    device.parent_id = Some(device_id::pci(address));
    device
}

fn storage_consumed_refs(devices: &[Device]) -> Vec<DeviceRef> {
    let mut refs = Vec::new();
    for device in devices {
        let Some(id) = device.parent_id.as_ref() else {
            continue;
        };
        if !refs.iter().any(|entry: &DeviceRef| entry.id == *id) {
            refs.push(DeviceRef { id: id.clone() });
        }
    }
    refs
}

fn apply_storage_vendor_fallback(mut device: Device) -> Device {
    if device.vendor.is_none() {
        device.vendor = device
            .model
            .as_deref()
            .or(Some(device.name.as_str()))
            .and_then(storage_vendor_from_model_prefix)
            .map(str::to_string);
    }
    device
}

fn storage_vendor_from_model_prefix(model: &str) -> Option<&'static str> {
    let model = model.trim().to_ascii_uppercase();
    [
        ("HGST HUS", "Western Digital"),
        ("WDC", "Western Digital"),
        ("HITACHI", "Hitachi"),
        ("HTS", "Hitachi"),
        ("IC", "Hitachi"),
        ("FUJITSU", "Fujitsu"),
        ("MP", "Fujitsu"),
        ("TOSHIBA", "Toshiba"),
        ("MK", "Toshiba"),
        ("MAXTOR", "Maxtor"),
        ("PIONEER", "Pioneer"),
        ("PHILIPS", "Philips"),
        ("QUANTUM", "Quantum"),
        ("FIREBALL", "Quantum"),
        ("FORESEE", "Foresee"),
        ("YMTC", "YMTC"),
        ("ZHITAI", "ZhiTai"),
        ("ZTC", "ZhiTai"),
        ("YEESTOR", "Yeestor"),
        ("MAXIO", "Maxio"),
        ("GLOWAY", "Gloway"),
        ("KINGSPEC", "KingSpec"),
        ("KINGSTON", "Kingston"),
        ("SANDISK", "SanDisk"),
        ("SAMSUNG", "Samsung"),
        ("MICRON", "Micron"),
        ("CT", "Crucial"),
        ("SKHYNIX", "SK hynix"),
        ("SK HYNIX", "SK hynix"),
        ("HYNIX", "SK hynix"),
        ("NETAC", "Netac"),
        ("RAMAXEL", "Ramaxel"),
        ("BIWIN", "Biwin"),
        ("CXMT", "CXMT"),
        ("TIGO", "Tigo"),
        ("COLORFUL", "Colorful"),
        ("ASGARD", "Asgard"),
        ("LEXAR", "Lexar"),
        ("IBM", "IBM"),
        ("RS", "Longsys"),
    ]
    .into_iter()
    .find_map(|(prefix, vendor)| model.starts_with(prefix).then_some(vendor))
    .or_else(|| storage_seagate_vendor_from_model_prefix(&model))
}

fn storage_seagate_vendor_from_model_prefix(model: &str) -> Option<&'static str> {
    let rest = model.strip_prefix("ST")?;
    rest.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
        .then_some("Seagate")
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
            let dmi = parse_dmidecode_bios_board(&result.stdout);
            let fallback = ProbeResult {
                devices: Vec::new(),
                warnings: vec![ScanWarning::new(
                    "source_empty",
                    "memory source produced no DIMM records",
                )
                .with_source(result.source.clone())],
                consumed: Vec::new(),
            };
            if let Some(device) = memory_array_device_from_dmi(dmi, &result.source) {
                return ProbeResult {
                    devices: vec![device],
                    warnings: fallback.warnings,
                    consumed: fallback.consumed,
                };
            }
            return memory_fallback_from_lshw_or_proc(ctx, fallback).await;
        }
        let lshw = memory_lshw_records_for_enrichment(ctx).await;
        let mut devices = memory_devices_from_records_with_lshw_enrichment(
            records,
            &result.source,
            SourceKind::Command,
            lshw.as_ref(),
        );
        if let Some(device) =
            memory_array_device_from_dmi(parse_dmidecode_bios_board(&result.stdout), &result.source)
        {
            devices.push(device);
        }
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

    let phytium_records = memory_records_from_phytium1500a_info_sysfs(ctx).await;
    if !phytium_records.is_empty() {
        fallback.devices = memory_devices_from_sysfs_records(phytium_records);
        return fallback;
    }

    if let Some((size_bytes, source)) = memory_total_from_device_tree(ctx).await {
        let device = Device::new(
            "memory:device-tree",
            DeviceKind::Memory,
            "Device Tree Memory",
            DeviceProperties::Memory(MemoryInfo {
                size_bytes: Some(size_bytes),
                ..Default::default()
            }),
        )
        .with_source(SourceEvidence {
            source,
            kind: SourceKind::Procfs,
            status: SourceStatus::Success,
            summary: None,
        });
        fallback.devices.push(device);
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

async fn memory_total_from_device_tree(ctx: &ProbeContext<'_>) -> Option<(u64, String)> {
    let mut paths = ctx
        .runner
        .glob("/proc/device-tree/memory@*/reg")
        .await
        .paths;
    paths.sort();
    let address_cells =
        read_device_tree_cell_count(ctx, "/proc/device-tree/#address-cells", 2).await;
    let size_cells = read_device_tree_cell_count(ctx, "/proc/device-tree/#size-cells", 1).await;
    if address_cells == 0 || size_cells == 0 {
        return None;
    }

    let mut total = 0u64;
    let mut sources = Vec::new();
    for path in paths {
        let result = ctx.runner.read_file_bytes(&path).await;
        if !result.is_success() {
            continue;
        }
        let size = parse_device_tree_memory_reg_size(&result.bytes, address_cells, size_cells)?;
        total = total.checked_add(size)?;
        sources.push(result.source);
    }

    (total > 0).then(|| {
        let source = if sources.len() == 1 {
            sources.remove(0)
        } else {
            "/proc/device-tree/memory@*/reg".to_string()
        };
        (total, source)
    })
}

async fn read_device_tree_cell_count(
    ctx: &ProbeContext<'_>,
    path: &str,
    default_value: usize,
) -> usize {
    let result = ctx.runner.read_file_bytes(Path::new(path)).await;
    if result.is_success() {
        parse_device_tree_u32(&result.bytes)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(default_value)
    } else {
        default_value
    }
}

fn parse_device_tree_memory_reg_size(
    bytes: &[u8],
    address_cells: usize,
    size_cells: usize,
) -> Option<u64> {
    let tuple_cells = address_cells.checked_add(size_cells)?;
    let tuple_bytes = tuple_cells.checked_mul(4)?;
    if tuple_bytes == 0 || bytes.len() % tuple_bytes != 0 {
        return None;
    }

    let mut total = 0u64;
    for tuple in bytes.chunks_exact(tuple_bytes) {
        let size_offset = address_cells * 4;
        let size = parse_device_tree_cell_value(&tuple[size_offset..], size_cells)?;
        total = total.checked_add(size)?;
    }
    Some(total)
}

fn parse_device_tree_cell_value(bytes: &[u8], cells: usize) -> Option<u64> {
    if cells > 2 || bytes.len() < cells * 4 {
        return None;
    }
    let mut value = 0u64;
    for chunk in bytes.chunks_exact(4).take(cells) {
        value = (value << 32) | u64::from(parse_device_tree_u32(chunk)?);
    }
    Some(value)
}

fn parse_device_tree_u32(bytes: &[u8]) -> Option<u32> {
    let bytes: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
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
        "/sys/bus/nvmem/devices/*/nvmem",
    ] {
        paths.extend(ctx.runner.glob(pattern).await.paths);
    }
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        warnings.push(
            ScanWarning::new(
                "source_missing",
                "no raw SPD EEPROM/nvmem sysfs paths found; load eeprom/ee1004/spd5118 kernel support if SPD fallback is required",
            )
            .with_source("raw SPD EEPROM sysfs"),
        );
    }

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
        if spd_record_is_partial_ddr5(&record) {
            warnings.push(
                ScanWarning::new(
                    "spd_partial",
                    "raw DDR5 SPD EEPROM identity was decoded without size or speed",
                )
                .with_source(result.source.clone()),
            );
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

fn spd_record_is_partial_ddr5(record: &DmiMemoryRecord) -> bool {
    record.memory_type.as_deref() == Some("DDR5 SDRAM")
        && (record.size.is_none() || record.speed.is_none())
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

async fn memory_records_from_phytium1500a_info_sysfs(
    ctx: &ProbeContext<'_>,
) -> Vec<SysfsMemoryRecord> {
    let mut paths = ctx
        .runner
        .glob("/sys/phytium1500a_info/memory*")
        .await
        .paths;
    paths.sort();

    let mut records = Vec::new();
    for path in paths {
        let result = ctx.runner.read_file(&path).await;
        if !result.is_success() {
            continue;
        }
        let record = parse_phytium1500a_memory_info(&result.stdout);
        if memory_record_has_data(&record) {
            records.push(SysfsMemoryRecord {
                record,
                source: result.source,
            });
        }
    }
    records
}

fn parse_phytium1500a_memory_info(input: &str) -> DmiMemoryRecord {
    let mut record = DmiMemoryRecord {
        memory_type: Some("DDR4".to_string()),
        data_width: Some("64 bits".to_string()),
        ..Default::default()
    };
    for line in input.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = clean_phytium1500a_value(value);
        match key.trim() {
            "Bank Locator" => record.locator = value,
            "Size" => record.size = value.and_then(clean_phytium1500a_size),
            "Manufacturer ID" => record.manufacturer = value.map(|value| value.to_uppercase()),
            _ => {}
        }
    }
    record
}

fn clean_phytium1500a_value(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != "$").then(|| value.to_string())
}

fn clean_phytium1500a_size(value: String) -> Option<String> {
    let first = value.split_whitespace().next().unwrap_or("");
    if first.len() > 9 && first.chars().all(|ch| ch.is_ascii_digit()) {
        None
    } else {
        Some(value)
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
    memory_devices_from_records_with_lshw_enrichment(records, source, source_kind, None)
}

struct MemoryLshwRecords {
    source: String,
    records: Vec<DmiMemoryRecord>,
}

async fn memory_lshw_records_for_enrichment(ctx: &ProbeContext<'_>) -> Option<MemoryLshwRecords> {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("lshw", ["-class", "memory"]), ctx.timeout)
        .await;
    result.is_success().then(|| {
        let records = parse_lshw_memory(&result.stdout);
        (!records.is_empty()).then_some(MemoryLshwRecords {
            source: result.source,
            records,
        })
    })?
}

fn memory_devices_from_records_with_lshw_enrichment(
    records: Vec<DmiMemoryRecord>,
    source: &str,
    source_kind: SourceKind,
    lshw: Option<&MemoryLshwRecords>,
) -> Vec<Device> {
    records
        .into_iter()
        .enumerate()
        .map(|(idx, mut mem)| {
            let lshw_contributed = lshw
                .and_then(|lshw| {
                    matching_lshw_memory_record(&mem, &lshw.records)
                        .map(|record| enrich_memory_record_from_lshw(&mut mem, record))
                })
                .unwrap_or(false);
            let mut device = memory_device_from_record(mem, idx, source, source_kind);
            if lshw_contributed {
                if let Some(lshw) = lshw {
                    device = device.with_source(SourceEvidence {
                        source: lshw.source.clone(),
                        kind: SourceKind::Command,
                        status: SourceStatus::Success,
                        summary: None,
                    });
                }
            }
            device
        })
        .collect()
}

fn matching_lshw_memory_record<'a>(
    dmi: &DmiMemoryRecord,
    records: &'a [DmiMemoryRecord],
) -> Option<&'a DmiMemoryRecord> {
    if let Some(locator) = dmi.locator.as_deref().and_then(memory_match_key) {
        if let Some(record) = records.iter().find(|record| {
            record
                .locator
                .as_deref()
                .and_then(memory_match_key)
                .is_some_and(|candidate| candidate == locator)
        }) {
            return Some(record);
        }
    }

    let serial = dmi.serial.as_deref().and_then(memory_match_key)?;
    records.iter().find(|record| {
        record
            .serial
            .as_deref()
            .and_then(memory_match_key)
            .is_some_and(|candidate| candidate == serial)
    })
}

fn memory_match_key(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != "--").then(|| value.to_ascii_lowercase())
}

fn enrich_memory_record_from_lshw(dmi: &mut DmiMemoryRecord, lshw: &DmiMemoryRecord) -> bool {
    let mut contributed = false;

    contributed |= replace_optional_string(&mut dmi.size, lshw.size.clone());
    contributed |= merge_optional_string(&mut dmi.name, lshw.name.clone());
    contributed |= merge_optional_string(&mut dmi.part_number, lshw.part_number.clone());
    if dmi.name.is_none() {
        dmi.name = dmi.part_number.clone();
    }
    contributed |= merge_optional_string(&mut dmi.manufacturer, lshw.manufacturer.clone());
    contributed |= merge_optional_string(&mut dmi.serial, lshw.serial.clone());
    contributed |= merge_optional_string(&mut dmi.locator, lshw.locator.clone());
    contributed |= merge_optional_string(&mut dmi.memory_type, lshw.memory_type.clone());
    contributed |= merge_optional_string(&mut dmi.speed, lshw.speed.clone());
    contributed |= merge_optional_string(&mut dmi.total_width, lshw.total_width.clone());
    contributed |= merge_optional_string(&mut dmi.data_width, lshw.data_width.clone());
    contributed |= merge_optional_string(&mut dmi.form_factor, lshw.form_factor.clone());

    contributed
}

fn replace_optional_string(target: &mut Option<String>, candidate: Option<String>) -> bool {
    if let Some(candidate) = candidate {
        if target.as_ref() != Some(&candidate) {
            *target = Some(candidate);
            return true;
        }
    }
    false
}

fn memory_array_device_from_dmi(dmi: DmiBiosBoardRecord, source: &str) -> Option<Device> {
    let has_array_data = dmi.memory_array_location.is_some()
        || dmi.memory_array_use.is_some()
        || dmi.memory_array_error_correction_type.is_some()
        || dmi.memory_array_maximum_capacity.is_some()
        || dmi.memory_array_error_information_handle.is_some()
        || dmi.memory_array_number_of_devices.is_some();
    has_array_data.then(|| {
        Device::new(
            "memory:array",
            DeviceKind::Memory,
            dmi.memory_array_use
                .clone()
                .unwrap_or_else(|| "Physical Memory Array".to_string()),
            DeviceProperties::Memory(MemoryInfo {
                size_bytes: parse_size_to_bytes(dmi.memory_array_maximum_capacity.as_deref()),
                memory_array_location: dmi.memory_array_location,
                memory_array_use: dmi.memory_array_use,
                memory_array_error_correction_type: dmi.memory_array_error_correction_type,
                memory_array_maximum_capacity_bytes: parse_size_to_bytes(
                    dmi.memory_array_maximum_capacity.as_deref(),
                ),
                memory_array_error_information_handle: dmi.memory_array_error_information_handle,
                memory_array_number_of_devices: dmi
                    .memory_array_number_of_devices
                    .as_deref()
                    .and_then(|value| value.parse().ok()),
                ..Default::default()
            }),
        )
        .with_source(SourceEvidence {
            source: source.to_string(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        })
    })
}

fn memory_device_from_record(
    mem: DmiMemoryRecord,
    idx: usize,
    source: &str,
    source_kind: SourceKind,
) -> Device {
    let vendor = mem.manufacturer.clone();
    let overview = memory_overview(
        mem.size.as_deref(),
        mem.part_number.as_deref().or(vendor.as_deref()),
        mem.memory_type.as_deref(),
        mem.speed.as_deref(),
    );
    let mem_info = memory_mem_info(
        mem.form_factor.as_deref(),
        mem.memory_type.as_deref(),
        mem.type_detail.as_deref(),
        mem.speed.as_deref(),
    );
    let rank = parse_memory_rank(mem.rank.as_deref());
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
    let name = memory_device_name(
        mem.name.as_deref(),
        mem.part_number.as_deref(),
        mem.locator.as_deref(),
        idx,
    );
    let mut device = Device::new(
        id,
        DeviceKind::Memory,
        name,
        DeviceProperties::Memory(MemoryInfo {
            size_bytes: parse_size_to_bytes(mem.size.as_deref()),
            vendor: mem.manufacturer,
            memory_type: mem.memory_type,
            speed_mtps: parse_speed_mtps(mem.speed.as_deref()),
            configured_speed_mtps: parse_speed_mtps(mem.configured_speed.as_deref()),
            total_width_bits: parse_width_bits(mem.total_width.as_deref()),
            data_width_bits: parse_width_bits(mem.data_width.as_deref()),
            min_voltage_v: parse_voltage_v(mem.minimum_voltage.as_deref()),
            max_voltage_v: parse_voltage_v(mem.maximum_voltage.as_deref()),
            configured_voltage_v: parse_voltage_v(mem.configured_voltage.as_deref()),
            locator: mem.locator,
            serial: mem.serial,
            part_number: mem.part_number,
            error_information_handle: mem.error_information_handle,
            form_factor: mem.form_factor,
            set: mem.set,
            bank_locator: mem.bank_locator,
            type_detail: mem.type_detail,
            asset_tag: mem.asset_tag,
            rank,
            module_manufacturer_id: mem.module_manufacturer_id,
            module_product_id: mem.module_product_id,
            memory_subsystem_controller_manufacturer_id: mem
                .memory_subsystem_controller_manufacturer_id,
            memory_subsystem_controller_product_id: mem.memory_subsystem_controller_product_id,
            memory_technology: mem.memory_technology,
            memory_operating_mode_capability: mem.memory_operating_mode_capability,
            firmware_version: mem.firmware_version,
            non_volatile_size_bytes: parse_size_to_bytes(mem.non_volatile_size.as_deref()),
            volatile_size_bytes: parse_size_to_bytes(mem.volatile_size.as_deref()),
            cache_size_bytes: parse_size_to_bytes(mem.cache_size.as_deref()),
            logical_size_bytes: parse_size_to_bytes(mem.logical_size.as_deref()),
            overview,
            mem_info,
            ..Default::default()
        }),
    );
    device.vendor = vendor;
    device.with_source(SourceEvidence {
        source: source.to_string(),
        kind: source_kind,
        status: SourceStatus::Success,
        summary: None,
    })
}

fn memory_device_name(
    name: Option<&str>,
    part_number: Option<&str>,
    locator: Option<&str>,
    idx: usize,
) -> String {
    [name, part_number, locator]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty() && *value != "--")
        .map(str::to_string)
        .unwrap_or_else(|| format!("Memory DIMM {idx}"))
}

fn parse_memory_rank(value: Option<&str>) -> Option<u32> {
    value?.split_whitespace().next()?.parse().ok()
}

fn memory_overview(
    size: Option<&str>,
    vendor: Option<&str>,
    memory_type: Option<&str>,
    speed: Option<&str>,
) -> Option<String> {
    let normalized_size;
    let size = size?.trim();
    if size.is_empty() {
        return None;
    }
    let size = match normalize_memory_display_size(size) {
        Some(value) => {
            normalized_size = value;
            normalized_size.as_str()
        }
        None => size,
    };
    let details = join_present([vendor, memory_type, speed]);
    Some(if details.is_empty() {
        size.to_string()
    } else {
        format!("{size}({details})")
    })
}

fn normalize_memory_display_size(value: &str) -> Option<String> {
    normalize_memory_display_size_for_arch(value, std::env::consts::ARCH)
}

fn normalize_memory_display_size_for_arch(value: &str, arch: &str) -> Option<String> {
    let first = value.split_whitespace().next()?;
    let (number, unit) = match first.parse::<u64>() {
        Ok(number) => (number, value.split_whitespace().nth(1)?),
        Err(_) => {
            let split = first.find(|ch: char| !ch.is_ascii_digit())?;
            if split == 0 {
                return None;
            }
            (first[..split].parse::<u64>().ok()?, &first[split..])
        }
    };
    match unit {
        "GiB" => Some(format!("{number} GB")),
        "MiB" => {
            let mut gib = (number + 512) / 1024;
            if arch == "sw_64" && gib % 2 != 0 {
                gib += 1;
            }
            (gib > 0).then(|| format!("{gib} GB"))
        }
        "MB" => {
            let gib = (number + 512) / 1024;
            (gib > 0).then(|| format!("{gib} GB"))
        }
        _ => None,
    }
}

fn memory_mem_info(
    form_factor: Option<&str>,
    memory_type: Option<&str>,
    type_detail: Option<&str>,
    speed: Option<&str>,
) -> Option<String> {
    let value = join_present([form_factor, memory_type, type_detail, speed]);
    (!value.is_empty()).then_some(value)
}

fn join_present<const N: usize>(parts: [Option<&str>; N]) -> String {
    parts
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|part| !part.is_empty() && *part != "--")
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod memory_tests {
    use super::*;

    #[test]
    fn sw64_memory_display_size_rounds_odd_gb_to_even() {
        assert_eq!(
            normalize_memory_display_size_for_arch("15 GiB", "sw_64").as_deref(),
            Some("15 GB")
        );
        assert_eq!(
            normalize_memory_display_size_for_arch("15360 MiB", "sw_64").as_deref(),
            Some("16 GB")
        );
        assert_eq!(
            normalize_memory_display_size_for_arch("15 GiB", "x86_64").as_deref(),
            Some("15 GB")
        );
    }
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
                    None,
                    Vec::new(),
                );
            }
            return fallback;
        }
        let mut dmi = parse_dmidecode_bios_board(&result.stdout);
        if dmi == Default::default() {
            if let Some(dmi) = read_sysfs_dmi(ctx).await {
                let runtime = read_bios_runtime_info(ctx).await;
                return ProbeResult {
                    devices: bios_board_devices(
                        dmi,
                        "/sys/class/dmi/id",
                        SourceKind::Sysfs,
                        runtime,
                        None,
                        None,
                        None,
                        Vec::new(),
                    ),
                    warnings: vec![ScanWarning::new(
                        "source_empty",
                        "bios source produced no DMI records; used sysfs fallback",
                    )
                    .with_source(result.source)],
                    consumed: Vec::new(),
                };
            }
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
        let chipset_source = enrich_dmi_chipset_family(ctx, &mut dmi).await;
        let oem_strings = read_dmi_oem_strings(ctx).await;
        let runtime = read_bios_runtime_info(ctx).await;
        ProbeResult::with_devices(bios_board_devices(
            dmi,
            &result.source,
            SourceKind::Command,
            runtime,
            bios_language_source,
            memory_array_source,
            chipset_source,
            oem_strings,
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

async fn read_dmi_oem_strings(ctx: &ProbeContext<'_>) -> Vec<String> {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("dmidecode", ["-t", "11"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return Vec::new();
    }
    parse_dmi_oem_strings(&result.stdout)
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

async fn enrich_dmi_chipset_family(
    ctx: &ProbeContext<'_>,
    dmi: &mut DmiBiosBoardRecord,
) -> Option<SourceEvidence> {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("lspci", ["-nn", "-k"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return None;
    }

    let chipset = parse_lspci_host_bridge_chipset(&result.stdout)?;
    dmi.chipset_family = Some(chipset);
    Some(SourceEvidence {
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
        chassis_manufacturer: read_sysfs_dmi_value(ctx, "chassis_vendor").await,
        chassis_type: read_sysfs_dmi_value(ctx, "chassis_type")
            .await
            .map(normalize_sysfs_chassis_type),
        chassis_version: read_sysfs_dmi_value(ctx, "chassis_version").await,
        chassis_serial: read_sysfs_dmi_value(ctx, "chassis_serial").await,
        chassis_asset_tag: read_sysfs_dmi_value(ctx, "chassis_asset_tag").await,
        ..Default::default()
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
    (!value.is_empty()
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "none"
                | "n/a"
                | "not specified"
                | "no asset tag"
                | "not settable"
                | "to be filled by o.e.m."
                | "system serial number"
        ))
    .then(|| value.to_string())
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

fn normalize_board_vendor_option(vendor: Option<String>) -> Option<String> {
    vendor.map(|value| {
        normalize_board_vendor(&value)
            .map(str::to_string)
            .unwrap_or(value)
    })
}

fn bios_board_devices(
    dmi: DmiBiosBoardRecord,
    source: &str,
    source_kind: SourceKind,
    runtime: BiosRuntimeInfo,
    bios_language_source: Option<SourceEvidence>,
    memory_array_source: Option<SourceEvidence>,
    chipset_source: Option<SourceEvidence>,
    oem_strings: Vec<String>,
) -> Vec<Device> {
    let bios_vendor = normalize_board_vendor_option(dmi.bios_vendor);
    let board_manufacturer = normalize_board_vendor_option(dmi.board_manufacturer);
    let chassis_manufacturer = normalize_board_vendor_option(dmi.chassis_manufacturer);
    let mut bios = Device::new(
        "bios:0",
        DeviceKind::Bios,
        dmi.bios_version
            .clone()
            .unwrap_or_else(|| "BIOS".to_string()),
        DeviceProperties::Bios(BiosInfo {
            vendor: bios_vendor,
            version: dmi.bios_version,
            release_date: dmi.bios_release_date,
            smbios_version: dmi.smbios_version,
            rom_size: dmi.bios_rom_size,
            runtime_size: dmi.bios_runtime_size,
            address: dmi.bios_address,
            characteristics: dmi.bios_characteristics,
            bios_revision: dmi.bios_revision,
            firmware_revision: dmi.firmware_revision,
            firmware_type: runtime.firmware_type,
            secure_boot: runtime.secure_boot,
            language_description_format: dmi.bios_language_description_format,
            installable_languages: dmi.bios_installable_languages,
            currently_installed_language: dmi.bios_currently_installed_language,
            oem_strings,
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
            manufacturer: board_manufacturer,
            product_name: dmi.board_product_name,
            version: dmi.board_version,
            serial: dmi.board_serial,
            asset_tag: dmi.board_asset_tag,
            chipset_family: dmi.chipset_family,
            board_features: dmi.board_features,
            board_type: dmi.board_type,
            location_in_chassis: dmi.board_location_in_chassis,
            chassis_handle: dmi.board_chassis_handle,
            chassis_manufacturer,
            chassis_type: dmi.chassis_type,
            chassis_version: dmi.chassis_version,
            chassis_serial: dmi.chassis_serial,
            chassis_asset_tag: dmi.chassis_asset_tag,
            chassis_lock: dmi.chassis_lock,
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
    let board = if let Some(source) = chipset_source {
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
        let nvidia_settings_memory_interface =
            gpu_nvidia_settings_memory_interface_record(ctx).await;
        let glxinfo = gpu_glxinfo_record(ctx).await;
        let proc_gpuinfo = gpu_proc_gpuinfo_record(ctx).await;
        let debug_gc_total_mem = gpu_debug_gc_total_mem_record(ctx).await;
        let xrandr = gpu_xrandr_record(ctx).await;
        let kylin_gpuinfo = gpu_kylin_gpuinfo_record(ctx).await;
        let enrichments = GpuEnrichmentSources {
            lshw: &lshw,
            drm: &drm,
            dmesg: &dmesg,
            nvidia_smi: &nvidia_smi,
            nvidia_settings: &nvidia_settings,
            nvidia_settings_memory_interface: &nvidia_settings_memory_interface,
            proc_gpuinfo: &proc_gpuinfo,
            debug_gc_total_mem: &debug_gc_total_mem,
            xrandr: &xrandr,
            kylin_gpuinfo: &kylin_gpuinfo,
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
        if gpus.is_empty() {
            let mut fallback = ProbeResult {
                devices: Vec::new(),
                warnings: vec![ScanWarning::new(
                    "source_empty",
                    "gpu source produced no display controller records",
                )
                .with_source(result.source)],
                consumed: Vec::new(),
            };
            fallback.devices =
                gpu_devices_from_sysfs_pci(ctx, &mut fallback.consumed, &enrichments).await;
            fallback.devices = apply_gpu_glxinfo_to_devices(fallback.devices, &glxinfo);
            return fallback;
        }
        let unique_gpu_count = gpus.len() == 1;
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
            let vid_pid = gpu_vid_pid(
                gpu.vendor_id.as_deref(),
                gpu.device_id.as_deref(),
                gpu.subsystem_vendor_id.as_deref(),
                gpu.subsystem_device_id.as_deref(),
            );
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
            });
            let driver_version = match gpu.kernel_driver.as_deref() {
                Some(driver) => gpu_driver_version(ctx, driver).await,
                None => None,
            };
            let device = device.with_driver(DriverInfo {
                name: gpu.kernel_driver.clone(),
                version: driver_version,
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
            probe_result.devices.push(apply_gpu_xrandr_enrichment(
                apply_gpu_kylin_gpuinfo_enrichment(
                    apply_gpu_pci_detail_enrichment(
                        ctx,
                        apply_gpu_memory_bus_width_enrichment(
                            apply_gpu_proc_gpuinfo_enrichment(
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
                                            unique_nvidia_settings_record(
                                                &nvidia_settings,
                                                use_nvidia_settings,
                                            )
                                        })
                                        .or_else(|| {
                                            unique_gpu_memory_record(
                                                &debug_gc_total_mem,
                                                unique_gpu_count,
                                            )
                                        }),
                                ),
                                &proc_gpuinfo,
                                jingjia_gpu_count == 1,
                            ),
                            unique_nvidia_settings_memory_interface_record(
                                &nvidia_settings_memory_interface,
                                use_nvidia_settings,
                            ),
                        ),
                        &address,
                        vid_pid,
                    )
                    .await,
                    &kylin_gpuinfo,
                    unique_gpu_count,
                ),
                &xrandr,
                unique_gpu_count,
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

#[derive(Default)]
struct GpuXrandrRecord {
    source: String,
    connectors: Vec<GpuConnectorInfo>,
    current_resolution: Option<String>,
    max_resolution: Option<String>,
    min_resolution: Option<String>,
}

#[derive(Default)]
struct KylinGpuinfoRecord {
    source: String,
    vendor: Option<String>,
    product: Option<String>,
}

struct GpuMemoryRecord {
    memory_bytes: u64,
    gddr_capacity: Option<String>,
    source: String,
    kind: SourceKind,
}

struct GpuMemoryBusWidthRecord {
    width_bits: u32,
    source: String,
    kind: SourceKind,
}

struct GpuEnrichmentSources<'a> {
    lshw: &'a GpuLshwDisplayRecords,
    drm: &'a GpuDrmRecords,
    dmesg: &'a GpuDmesgRecords,
    nvidia_smi: &'a GpuNvidiaSmiRecords,
    nvidia_settings: &'a Option<GpuMemoryRecord>,
    nvidia_settings_memory_interface: &'a Option<GpuMemoryBusWidthRecord>,
    proc_gpuinfo: &'a Option<GpuMemoryRecord>,
    debug_gc_total_mem: &'a Option<GpuMemoryRecord>,
    xrandr: &'a GpuXrandrRecord,
    kylin_gpuinfo: &'a KylinGpuinfoRecord,
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

async fn gpu_xrandr_record(ctx: &ProbeContext<'_>) -> GpuXrandrRecord {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("xrandr", ["--query"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return GpuXrandrRecord::default();
    }
    gpu_xrandr_record_from_query(result.source, parse_xrandr_query(&result.stdout))
}

async fn gpu_kylin_gpuinfo_record(ctx: &ProbeContext<'_>) -> KylinGpuinfoRecord {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("gpuinfo", std::iter::empty::<&str>()),
            ctx.timeout,
        )
        .await;
    if result.stdout.trim().is_empty() {
        return KylinGpuinfoRecord::default();
    }
    let mut record = KylinGpuinfoRecord {
        source: result.source,
        ..Default::default()
    };
    for line in result.stdout.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "GPU vendor" => record.vendor = Some(value.to_string()),
            "GPU type" => record.product = Some(value.to_string()),
            _ => {}
        }
    }
    record
}

fn gpu_xrandr_record_from_query(
    source: String,
    records: Vec<XrandrMonitorRecord>,
) -> GpuXrandrRecord {
    let connectors = records
        .iter()
        .map(|record| GpuConnectorInfo {
            connector: record.connector.clone(),
            interface: gpu_connector_interface(&record.connector),
            connected: record.connected,
            primary: record.primary,
            current_resolution: record.resolution.clone(),
            max_resolution: record.max_resolution.clone(),
        })
        .collect::<Vec<_>>();
    let current_resolution = records
        .iter()
        .find(|record| record.connected && record.primary)
        .or_else(|| records.iter().find(|record| record.connected))
        .and_then(|record| record.resolution.clone());
    let max_resolution = records
        .iter()
        .filter(|record| record.connected)
        .filter_map(|record| {
            let resolution = record
                .max_resolution
                .as_deref()
                .or(record.resolution.as_deref())?;
            Some((resolution_area(resolution)?, resolution.to_string()))
        })
        .max_by_key(|(area, _)| *area)
        .map(|(_, resolution)| resolution);
    let min_resolution = records
        .iter()
        .filter(|record| record.connected)
        .filter_map(|record| {
            let resolution = record.min_resolution.as_deref()?;
            Some((resolution_area(resolution)?, resolution.to_string()))
        })
        .min_by_key(|(area, _)| *area)
        .map(|(_, resolution)| resolution);
    GpuXrandrRecord {
        source,
        connectors,
        current_resolution,
        max_resolution,
        min_resolution,
    }
}

fn apply_gpu_xrandr_enrichment(
    mut device: Device,
    xrandr: &GpuXrandrRecord,
    use_for_device: bool,
) -> Device {
    if !use_for_device {
        return device;
    }
    let mut contributed = false;
    if let DeviceProperties::Gpu(gpu) = &mut device.properties {
        if gpu.current_resolution.is_none() && xrandr.current_resolution.is_some() {
            gpu.current_resolution = xrandr.current_resolution.clone();
            contributed = true;
        }
        if gpu.max_resolution.is_none() && xrandr.max_resolution.is_some() {
            gpu.max_resolution = xrandr.max_resolution.clone();
            contributed = true;
        }
        if gpu.min_resolution.is_none() && xrandr.min_resolution.is_some() {
            gpu.min_resolution = xrandr.min_resolution.clone();
            contributed = true;
        }
        if gpu.connectors.is_empty() && !xrandr.connectors.is_empty() {
            gpu.connectors = xrandr.connectors.clone();
            contributed = true;
        }
    }
    if contributed
        && !xrandr.source.is_empty()
        && !device
            .sources
            .iter()
            .any(|source| source.source == xrandr.source)
    {
        device = device.with_source(SourceEvidence {
            source: xrandr.source.clone(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

async fn gpu_driver_version(ctx: &ProbeContext<'_>, driver_name: &str) -> Option<String> {
    let sysfs_path = Path::new("/sys/module").join(driver_name).join("version");
    let sysfs_result = ctx.runner.read_file(&sysfs_path).await;
    if sysfs_result.is_success() {
        let value = sysfs_result.stdout.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    let modinfo_result = ctx
        .runner
        .run_command(&CommandSpec::new("modinfo", [driver_name]), ctx.timeout)
        .await;
    if modinfo_result.is_success() {
        return parse_modinfo_version(&modinfo_result.stdout);
    }
    None
}

fn apply_gpu_kylin_gpuinfo_enrichment(
    mut device: Device,
    record: &KylinGpuinfoRecord,
    use_for_device: bool,
) -> Device {
    if !use_for_device || (record.vendor.is_none() && record.product.is_none()) {
        return device;
    }
    let mut contributed = false;
    if let Some(product) = record.product.clone() {
        device.name = product.clone();
        device.model = Some(product);
        contributed = true;
    }
    let vendor = record
        .product
        .as_deref()
        .and_then(normalize_gpu_vendor)
        .map(str::to_string)
        .or_else(|| {
            record
                .vendor
                .as_deref()
                .and_then(normalize_gpu_vendor)
                .map(str::to_string)
        })
        .or_else(|| record.vendor.clone());
    if let Some(vendor) = vendor {
        device.vendor = Some(vendor.clone());
        if let DeviceProperties::Gpu(gpu) = &mut device.properties {
            gpu.vendor = Some(vendor);
        }
        contributed = true;
    }
    if contributed
        && !record.source.is_empty()
        && !device
            .sources
            .iter()
            .any(|source| source.source == record.source)
    {
        device = device.with_source(SourceEvidence {
            source: record.source.clone(),
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
    }
    device
}

fn gpu_connector_interface(connector: &str) -> Option<String> {
    let upper = connector.to_ascii_uppercase();
    if upper.starts_with("HDMI") {
        Some("HDMI".to_string())
    } else if upper.starts_with("EDP") {
        Some("eDP".to_string())
    } else if upper.starts_with("DP") || upper.starts_with("DISPLAYPORT") {
        Some("DP".to_string())
    } else if upper.starts_with("VGA") {
        Some("VGA".to_string())
    } else if upper.starts_with("DVI") {
        Some("DVI".to_string())
    } else {
        None
    }
}

fn resolution_area(resolution: &str) -> Option<u64> {
    let (width, height) = resolution.split_once('x')?;
    Some(width.parse::<u64>().ok()? * height.parse::<u64>().ok()?)
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
        if gpu.glsl_version.is_none() && record.glsl_version.is_some() {
            gpu.glsl_version = record.glsl_version.clone();
            contributed = true;
        }
        if gpu.egl_version.is_none() && record.egl_version.is_some() {
            gpu.egl_version = record.egl_version.clone();
            contributed = true;
        }
        if gpu.egl_client_apis.is_none() && record.egl_client_apis.is_some() {
            gpu.egl_client_apis = record.egl_client_apis.clone();
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
        gddr_capacity: parse_deepin_gpu_info_gddr_capacity(&result.stdout),
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
        gddr_capacity: None,
        source: result.source,
        kind: SourceKind::Procfs,
    })
}

async fn gpu_debug_gc_total_mem_record(ctx: &ProbeContext<'_>) -> Option<GpuMemoryRecord> {
    let path = Path::new("/sys/kernel/debug/gc/total_mem");
    let result = ctx.runner.read_file(path).await;
    if !result.is_success() {
        return None;
    }
    Some(GpuMemoryRecord {
        memory_bytes: parse_gpu_debug_gc_total_mem(&result.stdout)?,
        gddr_capacity: None,
        source: result.source,
        kind: SourceKind::Sysfs,
    })
}

fn parse_gpu_debug_gc_total_mem(input: &str) -> Option<u64> {
    let line = input.lines().find(|line| !line.trim().is_empty())?;
    let mut parts = line.split_whitespace();
    let value = parts.next()?.parse::<f64>().ok()?;
    let unit = parts
        .next()
        .unwrap_or("B")
        .trim_matches(|ch| ch == '(' || ch == ')');
    let multiplier = match unit.to_ascii_lowercase().as_str() {
        "kb" | "kib" => 1024.0,
        "mb" | "mib" => 1024.0 * 1024.0,
        "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
        "b" => 1.0,
        _ => return None,
    };
    Some((value * multiplier) as u64)
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

fn parse_deepin_gpu_info_gddr_capacity(input: &str) -> Option<String> {
    input.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case("GDDR capacity")
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
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
        if gpu.gddr_capacity.is_none() && record.gddr_capacity.is_some() {
            gpu.gddr_capacity = record.gddr_capacity.clone();
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

async fn apply_gpu_pci_detail_enrichment(
    ctx: &ProbeContext<'_>,
    mut device: Device,
    address: &str,
    vid_pid: Option<String>,
) -> Device {
    let modalias = read_optional_trimmed(
        ctx,
        &Path::new("/sys/bus/pci/devices")
            .join(address)
            .join("modalias"),
    )
    .await;
    let vid_pid = modalias
        .as_deref()
        .and_then(gpu_vid_pid_from_modalias)
        .or(vid_pid);
    let mut contributed = false;
    if let DeviceProperties::Gpu(gpu) = &mut device.properties {
        if gpu.vid_pid.is_none() && vid_pid.is_some() {
            gpu.vid_pid = vid_pid.clone();
            gpu.phys_id = gpu.phys_id.take().or(vid_pid);
            contributed = true;
        }
        if gpu.modalias.is_none() && modalias.is_some() {
            gpu.modalias = modalias;
            contributed = true;
        }
    }
    if contributed {
        let source = format!("/sys/bus/pci/devices/{address}");
        if !device
            .sources
            .iter()
            .any(|source_evidence| source_evidence.source == source)
        {
            device = device.with_source(SourceEvidence {
                source,
                kind: SourceKind::Sysfs,
                status: SourceStatus::Success,
                summary: None,
            });
        }
    }
    device
}

fn gpu_vid_pid(
    vendor_id: Option<&str>,
    device_id: Option<&str>,
    subsystem_vendor_id: Option<&str>,
    subsystem_device_id: Option<&str>,
) -> Option<String> {
    let vendor_id = vendor_id?;
    let device_id = device_id?;
    Some(
        [
            Some(vendor_id),
            Some(device_id),
            subsystem_vendor_id,
            subsystem_device_id,
        ]
        .into_iter()
        .flatten()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("/"),
    )
}

fn gpu_vid_pid_from_modalias(modalias: &str) -> Option<String> {
    let (_, after_vendor_marker) = modalias.split_once(":v")?;
    let vendor = after_vendor_marker.get(..8)?;
    let (_, after_device_marker) = after_vendor_marker.get(8..)?.split_once('d')?;
    let device = after_device_marker.get(..8)?;
    let subsystem_vendor = after_device_marker
        .get(8..)
        .and_then(|tail| tail.split_once("sv"))
        .and_then(|(_, tail)| tail.get(..8));
    let subsystem_device = after_device_marker
        .get(8..)
        .and_then(|tail| tail.split_once("sd"))
        .and_then(|(_, tail)| tail.get(..8));
    Some(
        [
            Some(vendor),
            Some(device),
            subsystem_vendor,
            subsystem_device,
        ]
        .into_iter()
        .flatten()
        .map(|value| value.trim_start_matches('0').to_ascii_lowercase())
        .map(|value| {
            if value.is_empty() {
                "0".to_string()
            } else {
                value
            }
        })
        .collect::<Vec<_>>()
        .join("/"),
    )
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
    if let DeviceProperties::Gpu(gpu) = &mut device.properties {
        if gpu.memory_bus_width_bits.is_none() && record.width_bits.is_some() {
            gpu.memory_bus_width_bits = record.width_bits;
            contributed = true;
        }
        if gpu.clock_mhz.is_none() && record.clock_mhz.is_some() {
            gpu.clock_mhz = record.clock_mhz;
            contributed = true;
        }
        if gpu.irq.is_none() && record.irq.is_some() {
            gpu.irq = record.irq.clone();
            contributed = true;
        }
        if gpu.capabilities.is_empty() && !record.capabilities.is_empty() {
            gpu.capabilities = record.capabilities.clone();
            contributed = true;
        }
        if gpu.io_port.is_none() && record.io_port.is_some() {
            gpu.io_port = record.io_port.clone();
            contributed = true;
        }
        if gpu.mem_address.is_none() && record.mem_address.is_some() {
            gpu.mem_address = record.mem_address.clone();
            contributed = true;
        }
        if gpu.revision.is_none() && record.version.is_some() {
            gpu.revision = record.version.clone();
            contributed = true;
        }
        if gpu.description.is_none() && record.description.is_some() {
            gpu.description = record.description.clone();
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
                    gddr_capacity: None,
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
        gddr_capacity: None,
        source: result.source,
        kind: SourceKind::Command,
    })
}

async fn gpu_nvidia_settings_memory_interface_record(
    ctx: &ProbeContext<'_>,
) -> Option<GpuMemoryBusWidthRecord> {
    let result = ctx
        .runner
        .run_command(
            &CommandSpec::new("nvidia-settings", ["-q", "GPUMemoryInterface"]),
            ctx.timeout,
        )
        .await;
    if !result.is_success() {
        return None;
    }

    Some(GpuMemoryBusWidthRecord {
        width_bits: parse_nvidia_settings_memory_interface(&result.stdout)?,
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

fn unique_nvidia_settings_memory_interface_record(
    record: &Option<GpuMemoryBusWidthRecord>,
    use_for_device: bool,
) -> Option<&GpuMemoryBusWidthRecord> {
    use_for_device.then_some(record.as_ref()).flatten()
}

fn apply_gpu_memory_bus_width_enrichment(
    mut device: Device,
    record: Option<&GpuMemoryBusWidthRecord>,
) -> Device {
    let Some(record) = record else {
        return device;
    };
    let mut contributed = false;
    if let DeviceProperties::Gpu(gpu) = &mut device.properties {
        if gpu.memory_bus_width_bits.is_none() {
            gpu.memory_bus_width_bits = Some(record.width_bits);
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

fn unique_gpu_memory_record(
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
    let unique_gpu_count = records.len() == 1;

    for record in records {
        let address = record.address.clone();
        let use_nvidia_settings = nvidia_gpu_count == 1
            && is_nvidia_gpu_identity(record.vendor_id.as_deref(), None, None);
        let vid_pid = gpu_vid_pid(
            record.vendor_id.as_deref(),
            record.device_id.as_deref(),
            record.subsystem_vendor_id.as_deref(),
            record.subsystem_device_id.as_deref(),
        );
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
        devices.push(apply_gpu_xrandr_enrichment(
            apply_gpu_kylin_gpuinfo_enrichment(
                apply_gpu_pci_detail_enrichment(
                    ctx,
                    apply_gpu_memory_bus_width_enrichment(
                        apply_gpu_proc_gpuinfo_enrichment(
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
                                        unique_nvidia_settings_record(
                                            sources.nvidia_settings,
                                            use_nvidia_settings,
                                        )
                                    })
                                    .or_else(|| {
                                        unique_gpu_memory_record(
                                            sources.debug_gc_total_mem,
                                            unique_gpu_count,
                                        )
                                    }),
                            ),
                            sources.proc_gpuinfo,
                            jingjia_gpu_count == 1,
                        ),
                        unique_nvidia_settings_memory_interface_record(
                            sources.nvidia_settings_memory_interface,
                            use_nvidia_settings,
                        ),
                    ),
                    &address,
                    vid_pid,
                )
                .await,
                sources.kylin_gpuinfo,
                unique_gpu_count,
            ),
            sources.xrandr,
            unique_gpu_count,
        ));
    }

    devices
}

struct MonitorProbeEntry {
    connector: String,
    resolution: Option<String>,
    current_refresh_hz: Option<u16>,
    primary: bool,
    max_resolution: Option<String>,
    support_resolutions: Vec<String>,
    source: String,
    source_kind: SourceKind,
    require_edid: bool,
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
        let mut edids: HashMap<String, Vec<(Vec<u8>, String, String)>> = HashMap::new();
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
                    let hex = bytes_to_hex(&bytes_result.bytes);
                    edids
                        .entry(connector)
                        .or_default()
                        .push((bytes_result.bytes, bytes_result.source, hex));
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
                let hex = bytes_to_hex(&bytes);
                edids.entry(connector).or_default().push((bytes, source, hex));
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
                let hex = bytes_to_hex(&bytes);
                edids.entry(connector).or_default().push((bytes, source, hex));
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
                let hex = bytes_to_hex(&bytes);
                edids.entry(connector).or_default().push((bytes, source, hex));
                continue;
            }

            edids.entry(connector).or_default().extend(
                readable.into_iter().map(|(bytes, source, _)| {
                    let hex = bytes_to_hex(&bytes);
                    (bytes, source, hex)
                }),
            );
        }
        if verbose_result.is_success() {
            for record in parse_xrandr_verbose(&verbose_result.stdout) {
                edids.entry(record.connector).or_default().insert(
                    0,
                    (record.edid, verbose_result.source.clone(), record.edid_hex),
                );
            }
        }

        let mut monitors: Vec<MonitorProbeEntry> = if result.is_success() {
            let records = parse_xrandr_query(&result.stdout);
            if records.is_empty() {
                warnings.push(
                    ScanWarning::new("source_empty", "monitor source produced no xrandr records")
                        .with_source(result.source.clone()),
                );
                edids
                    .keys()
                    .map(|connector| MonitorProbeEntry {
                        connector: connector.clone(),
                        resolution: None,
                        current_refresh_hz: None,
                        primary: false,
                        max_resolution: None,
                        support_resolutions: Vec::new(),
                        source: result.source.clone(),
                        source_kind: SourceKind::Command,
                        require_edid: true,
                    })
                    .collect()
            } else {
                records
                    .into_iter()
                    .filter(|mon| mon.connected)
                    .map(|mon| MonitorProbeEntry {
                        connector: mon.connector,
                        resolution: mon.resolution,
                        current_refresh_hz: mon.current_refresh_hz,
                        primary: mon.primary,
                        max_resolution: mon.max_resolution,
                        support_resolutions: mon.support_resolutions,
                        source: result.source.clone(),
                        source_kind: SourceKind::Command,
                        require_edid: false,
                    })
                    .collect()
            }
        } else {
            warnings.extend(ProbeResult::source_failure(self.name(), &result).warnings);
            edids
                .keys()
                .map(|connector| MonitorProbeEntry {
                    connector: connector.clone(),
                    resolution: None,
                    current_refresh_hz: None,
                    primary: false,
                    max_resolution: None,
                    support_resolutions: Vec::new(),
                    source: result.source.clone(),
                    source_kind: SourceKind::Command,
                    require_edid: true,
                })
                .collect()
        };
        monitors.sort_by(|left, right| left.connector.cmp(&right.connector));

        let mut devices: Vec<_> = monitors
            .into_iter()
            .filter_map(
                |MonitorProbeEntry {
                     connector,
                     resolution,
                     current_refresh_hz,
                     primary,
                     max_resolution,
                     support_resolutions,
                     mut source,
                     mut source_kind,
                     require_edid,
                 }| {
                    let id = device_id::other("monitor", &connector);
                    let mut info = MonitorInfo {
                        connector: Some(connector.clone()),
                        interface: monitor_connector_interface(&connector),
                        raw_interface: Some(connector.clone()),
                        aspect_ratio: resolution.as_deref().and_then(monitor_aspect_ratio),
                        resolution,
                        current_refresh_hz,
                        is_primary: primary,
                        max_resolution,
                        support_resolutions,
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
            aspect_ratio: record.resolution.as_deref().and_then(monitor_aspect_ratio),
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
        info.aspect_ratio = info.resolution.as_deref().and_then(monitor_aspect_ratio);
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
    candidates: &[(Vec<u8>, String, String)],
    warnings: &mut Vec<ScanWarning>,
) -> Option<String> {
    for (bytes, source, hex) in candidates
        .iter()
        .filter(|(_, source, _)| !is_sysfs_edid_source(source))
    {
        match parse_edid(bytes) {
            Ok(edid) => {
                apply_edid(info, edid);
                info.edid_hex = Some(hex.clone());
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

    let sysfs_count = candidates
        .iter()
        .filter(|(_, source, _)| is_sysfs_edid_source(source))
        .count();
    if sysfs_count > 1 {
        return apply_unique_resolution_matched_edid(info, id, candidates, warnings);
    }

    for (bytes, source, hex) in candidates
        .iter()
        .filter(|(_, source, _)| is_sysfs_edid_source(source))
    {
        match parse_edid(bytes) {
            Ok(edid) => {
                apply_edid(info, edid);
                info.edid_hex = Some(hex.clone());
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

fn apply_unique_resolution_matched_edid(
    info: &mut MonitorInfo,
    id: &str,
    candidates: &[(Vec<u8>, String, String)],
    warnings: &mut Vec<ScanWarning>,
) -> Option<String> {
    let (width, height) = info
        .resolution
        .as_deref()
        .and_then(parse_monitor_resolution)?;
    let mut matched = None;
    for (bytes, source, hex) in candidates
        .iter()
        .filter(|(_, source, _)| is_sysfs_edid_source(source))
    {
        match parse_edid(bytes) {
            Ok(edid) if edid_preferred_mode_matches(&edid, width, height) => {
                if matched.is_some() {
                    return None;
                }
                matched = Some((edid, source.clone(), hex.clone()));
            }
            Ok(_) => {}
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
    let (edid, source, hex) = matched?;
    apply_edid(info, edid);
    info.edid_hex = Some(hex);
    Some(source)
}

fn parse_monitor_resolution(value: &str) -> Option<(u16, u16)> {
    let (width, height) = value.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

fn is_sysfs_edid_source(source: &str) -> bool {
    source.starts_with("/sys/")
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", byte);
    }
    out
}

fn edid_preferred_mode_matches(edid: &hw_parser::EdidRecord, width: u16, height: u16) -> bool {
    edid.preferred_mode
        .as_ref()
        .is_some_and(|mode| mode.width == width && mode.height == height)
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
    info.production_date = monitor_production_date(edid.year, edid.week);
    info.size_cm = edid.size_cm;
    if info.size_mm.is_none() {
        info.size_mm = edid.size_mm;
    }
    info.diagonal_inches = edid.size_cm.map(diagonal_inches);
    info.gamma = edid.gamma;
    info.preferred_width = edid.preferred_mode.as_ref().map(|mode| mode.width);
    info.preferred_height = edid.preferred_mode.as_ref().map(|mode| mode.height);
    info.preferred_refresh_hz = edid.preferred_mode.as_ref().map(|mode| mode.refresh_hz);
}

fn monitor_connector_interface(connector: &str) -> Option<String> {
    let upper = connector.to_ascii_uppercase();
    if upper.starts_with("HDMI") {
        Some("HDMI".to_string())
    } else if upper.starts_with("EDP") {
        Some("eDP".to_string())
    } else if upper.starts_with("DP") || upper.starts_with("DISPLAYPORT") {
        Some("DP".to_string())
    } else if upper.starts_with("VGA") {
        Some("VGA".to_string())
    } else if upper.starts_with("DVI") {
        Some("DVI".to_string())
    } else {
        None
    }
}

fn monitor_production_date(year: Option<u16>, week: Option<u8>) -> Option<String> {
    let year = year?;
    let week = week?;
    if week == 0 {
        return None;
    }
    let month = ((week as u16 * 7).saturating_sub(1) / 30 + 1).min(12);
    Some(format!("{year:04}-{month:02}"))
}

fn monitor_aspect_ratio(resolution: &str) -> Option<String> {
    let (width, height) = parse_monitor_resolution(resolution)?;
    let width = width as u32;
    let height = height as u32;
    if width == 0 || height == 0 {
        return None;
    }
    let ratio = width as f32 / height as f32;
    if (ratio - 21.0 / 9.0).abs() < 0.08 {
        return Some("21:9".to_string());
    }
    if (ratio - 32.0 / 9.0).abs() < 0.08 {
        return Some("32:9".to_string());
    }
    let divisor = gcd(width, height);
    Some(format!("{}:{}", width / divisor, height / divisor))
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    left
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
