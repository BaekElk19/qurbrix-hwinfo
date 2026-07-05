use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, BiosInfo, BusInfo, CpuInfo, Device, DeviceKind, DeviceProperties, DeviceRef,
    DriverInfo, DriverStatus, GpuInfo, MemoryInfo, MonitorInfo, MotherboardInfo, NetworkInfo,
    SourceEvidence, SourceKind, SourceStatus, StorageInfo,
};
use hw_parser::{
    parse_dmidecode_bios_board, parse_dmidecode_memory, parse_gpu_lspci, parse_ip_j_link,
    parse_lsblk_json, parse_lscpu, parse_size_to_bytes, parse_speed_mtps, parse_xrandr_query,
};
use hw_source::CommandSpec;

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
        let result = ctx
            .runner
            .run_command(
                &CommandSpec::new("lscpu", std::iter::empty::<&str>()),
                ctx.timeout,
            )
            .await;
        if !result.is_success() {
            return ProbeResult::source_failure(self.name(), &result);
        }
        let cpu = parse_lscpu(&result.stdout);
        let cores = match (cpu.cores_per_socket, cpu.sockets) {
            (Some(cores), Some(sockets)) => Some(cores * sockets),
            _ => None,
        };
        let device = Device::new(
            "cpu:0",
            DeviceKind::Cpu,
            cpu.model_name.clone().unwrap_or_else(|| "CPU".to_string()),
            DeviceProperties::Cpu(CpuInfo {
                name: cpu.model_name,
                vendor: cpu.vendor,
                architecture: cpu.architecture,
                cores,
                threads: cpu.threads,
                sockets: cpu.sockets,
                ..Default::default()
            }),
        )
        .with_source(SourceEvidence {
            source: result.source,
            kind: SourceKind::Command,
            status: SourceStatus::Success,
            summary: None,
        });
        ProbeResult::with_devices(vec![device])
    }
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
        let devices = parse_ip_j_link(&result.stdout)
            .into_iter()
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
        let devices = parse_lsblk_json(&result.stdout)
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
            probe_result.consumed.push(DeviceRef { id: pci_id });
            probe_result.devices.push(
                Device::new(
                    device_id::other("gpu:pci", &gpu.address),
                    DeviceKind::Gpu,
                    gpu.device
                        .clone()
                        .or(gpu.vendor.clone())
                        .unwrap_or_else(|| "GPU".to_string()),
                    DeviceProperties::Gpu(GpuInfo::default()),
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
        if !result.is_success() {
            return ProbeResult::source_failure(self.name(), &result);
        }
        let devices = parse_xrandr_query(&result.stdout)
            .into_iter()
            .filter(|mon| mon.connected)
            .map(|mon| {
                Device::new(
                    device_id::other("monitor", &mon.connector),
                    DeviceKind::Monitor,
                    mon.connector.clone(),
                    DeviceProperties::Monitor(MonitorInfo {
                        connector: Some(mon.connector),
                        resolution: mon.resolution,
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
