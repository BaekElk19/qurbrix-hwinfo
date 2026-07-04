use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, CpuInfo, Device, DeviceKind, DeviceProperties, NetworkInfo, SourceEvidence,
    SourceKind, SourceStatus, StorageInfo,
};
use hw_parser::{parse_ip_j_link, parse_lsblk_json, parse_lscpu};
use hw_source::CommandSpec;

pub struct CpuProbe;
pub struct NetworkProbe;
pub struct StorageProbe;

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
            return ProbeResult::default();
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
            return ProbeResult::default();
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
            return ProbeResult::default();
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
