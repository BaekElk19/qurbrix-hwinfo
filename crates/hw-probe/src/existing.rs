use crate::{Probe, ProbeContext, ProbeResult};
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
    parse_gpu_lspci, parse_ip_j_link_result, parse_lsblk_json_result, parse_lscpu,
    parse_lshw_processor, parse_proc_cpuinfo, parse_size_to_bytes, parse_speed_mtps,
    parse_xrandr_query, parse_xrandr_verbose,
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
        let lscpu_contributed = lscpu.is_some();
        let lshw_contributed = lshw.is_some();
        let dmi_contributed = !dmi.is_empty();
        let proc_cpuinfo_contributed = proc_cpuinfo.is_some();
        if lscpu.is_none() && lshw.is_none() && dmi.is_empty() && proc_cpuinfo.is_none() {
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
        let merged = merge_cpu_records(merge_cpu_record_fallback(lscpu, proc_cpuinfo), lshw, &dmi);
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
            return ProbeResult::source_failure(self.name(), &result);
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
        let devices = records
            .into_iter()
            .filter(|net| !is_ignored_network_interface(&net.ifname))
            .map(|net| {
                Device::new(
                    device_id::network(net.address.as_deref(), &net.ifname),
                    DeviceKind::Network,
                    net.ifname.clone(),
                    DeviceProperties::Network(NetworkInfo {
                        interface: Some(net.ifname),
                        mac: net.address,
                        operstate: net.operstate,
                        ..Default::default()
                    }),
                )
                .with_source(SourceEvidence {
                    source: result.source.clone(),
                    kind: SourceKind::Command,
                    status: SourceStatus::Success,
                    summary: None,
                })
            })
            .collect();
        ProbeResult::with_devices(devices)
    }
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
                    ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN"],
                ),
                ctx.timeout,
            )
            .await;
        if !result.is_success() {
            return ProbeResult::source_failure(self.name(), &result);
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
        let devices = records
            .into_iter()
            .filter(|dev| dev.device_type.as_deref() == Some("disk"))
            .map(|dev| {
                let node = format!("/dev/{}", dev.name);
                Device::new(
                    device_id::storage(None, dev.serial.as_deref(), &node),
                    DeviceKind::Storage,
                    dev.model.clone().unwrap_or_else(|| node.clone()),
                    DeviceProperties::Storage(StorageInfo {
                        device_node: Some(node),
                        size_bytes: dev.size,
                        media_type: dev.tran,
                        ..Default::default()
                    }),
                )
                .with_source(SourceEvidence {
                    source: result.source.clone(),
                    kind: SourceKind::Command,
                    status: SourceStatus::Success,
                    summary: None,
                })
            })
            .collect();
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
            return ProbeResult::source_failure(self.name(), &result);
        }
        let devices = parse_dmidecode_memory(&result.stdout)
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
                    source: result.source.clone(),
                    kind: SourceKind::Command,
                    status: SourceStatus::Success,
                    summary: None,
                })
            })
            .collect();
        ProbeResult::with_devices(devices)
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
            return ProbeResult::source_failure(self.name(), &result);
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
            source: result.source.clone(),
            kind: SourceKind::Command,
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
            source: result.source,
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
        ProbeResult::with_devices(vec![bios, board])
    }
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
        let result = ctx
            .runner
            .run_command(&CommandSpec::new("lspci", ["-nn", "-k"]), ctx.timeout)
            .await;
        if !result.is_success() {
            return ProbeResult::source_failure(self.name(), &result);
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
            probe_result.devices.push(
                Device::new(
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
                }),
            );
        }
        probe_result
    }
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
