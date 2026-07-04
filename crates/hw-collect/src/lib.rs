use anyhow::Result;
use hw_model::{
    BiosInfo, CpuInfo, Inventory, MemoryInfo, MonitorInfo, NetInfo, ParseOutput, StorageInfo,
};
use hw_parser::{
    parse_bios, parse_bios_from_raw, parse_cpu, parse_cpu_from_raw, parse_gpu, parse_memory,
    parse_memory_from_raw, parse_monitors, parse_monitors_from_raw, parse_net, parse_net_from_raw,
    parse_storage, parse_storage_from_raw,
};
use hw_source::{CmdSource, CmdSpec, Source};
use std::collections::HashMap;
use tokio::{fs, task};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct CollectConfig {
    pub command_timeout_ms: u64,
    pub monitor: MonitorCollectConfig,
    pub net: NetCollectConfig,
    pub gpu: GpuCollectConfig,
}

impl Default for CollectConfig {
    fn default() -> Self {
        Self {
            command_timeout_ms: 5_000,
            monitor: MonitorCollectConfig::default(),
            net: NetCollectConfig::default(),
            gpu: GpuCollectConfig::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonitorCollectConfig {
    pub enable: bool,
    pub timeout_ms: u64,
    pub enable_xrandr: bool,
}

impl Default for MonitorCollectConfig {
    fn default() -> Self {
        Self {
            enable: true,
            timeout_ms: 5_000,
            enable_xrandr: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetCollectConfig {
    pub enable: bool,
    pub timeout_ms: u64,
    pub enable_lspci: bool,
    pub enable_ethtool: bool,
}

impl Default for NetCollectConfig {
    fn default() -> Self {
        Self {
            enable: true,
            timeout_ms: 5_000,
            enable_lspci: true,
            enable_ethtool: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GpuCollectConfig {
    pub enable: bool,
    pub timeout_ms: u64,
    pub enable_glxinfo: bool,
    pub enable_xrandr: bool,
}

impl Default for GpuCollectConfig {
    fn default() -> Self {
        Self {
            enable: true,
            timeout_ms: 5_000,
            enable_glxinfo: true,
            enable_xrandr: true,
        }
    }
}

pub async fn collect_inventory(config: &CollectConfig) -> Result<Inventory> {
    info!("collecting cpu information");
    let cpu = collect_cpu_component(config).await?;

    info!("collecting memory information");
    let memory = collect_memory_component(config).await?;

    info!("collecting bios information");
    let bios = collect_bios_component(config).await?;

    info!("collecting monitor information");
    let monitors = collect_monitor_component(config).await?;

    info!("collecting storage information");
    let storage = collect_storage_component(config).await?;

    info!("collecting gpu information");
    let gpus = if config.gpu.enable {
        task::spawn_blocking(parse_gpu).await??
    } else {
        Vec::new()
    };

    info!("collecting network information");
    let networks = collect_net_component(config).await?;

    Ok(Inventory {
        cpu,
        memory,
        bios,
        monitors,
        storage,
        gpus,
        networks,
    })
}

async fn collect_cpu_component(config: &CollectConfig) -> Result<ParseOutput<CpuInfo>> {
    let source = CmdSource::new(
        vec![
            CmdSpec::new("lscpu", "lscpu", Vec::new()),
            CmdSpec::new("proc_cpuinfo", "cat", vec!["/proc/cpuinfo".into()]),
        ],
        config.command_timeout_ms,
    );

    match source.collect().await {
        Ok(outputs) => {
            info!(outputs = outputs.len(), "cpu command outputs collected");
            let lscpu_raw = outputs.get("lscpu").map(|s| s.as_str()).unwrap_or("");
            let cpuinfo_raw = outputs
                .get("proc_cpuinfo")
                .map(|s| s.as_str())
                .unwrap_or("");
            if lscpu_raw.is_empty() && cpuinfo_raw.is_empty() {
                warn!("cpu command outputs empty, falling back to direct parser");
                fallback_cpu().await
            } else {
                parse_cpu_from_raw(lscpu_raw, cpuinfo_raw)
            }
        }
        Err(err) => {
            warn!(error = %err, "cpu command collection failed, falling back");
            fallback_cpu().await
        }
    }
}

async fn collect_memory_component(config: &CollectConfig) -> Result<Vec<ParseOutput<MemoryInfo>>> {
    let meminfo_raw = fs::read_to_string("/proc/meminfo")
        .await
        .unwrap_or_default();
    let source = CmdSource::new(
        vec![CmdSpec::new(
            "dmidecode_memory",
            "dmidecode",
            vec!["-t".into(), "memory".into()],
        )],
        config.command_timeout_ms,
    );

    match source.collect().await {
        Ok(outputs) => {
            info!(outputs = outputs.len(), "memory command outputs collected");
            let dmidecode_raw = outputs
                .get("dmidecode_memory")
                .map(|s| s.as_str())
                .unwrap_or("");
            if dmidecode_raw.is_empty() {
                warn!("dmidecode memory output empty, falling back to direct parser");
                fallback_memory().await
            } else {
                parse_memory_from_raw(dmidecode_raw, Some(meminfo_raw.as_str()))
            }
        }
        Err(err) => {
            warn!(error = %err, "memory command collection failed, falling back");
            fallback_memory().await
        }
    }
}

async fn collect_bios_component(config: &CollectConfig) -> Result<ParseOutput<BiosInfo>> {
    let source = CmdSource::new(
        vec![CmdSpec::new(
            "dmidecode_bios",
            "dmidecode",
            vec![
                "-t".into(),
                "bios".into(),
                "-t".into(),
                "system".into(),
                "-t".into(),
                "baseboard".into(),
                "-t".into(),
                "chassis".into(),
                "-t".into(),
                "memory".into(),
            ],
        )],
        config.command_timeout_ms,
    );

    match source.collect().await {
        Ok(outputs) => {
            info!(outputs = outputs.len(), "bios command outputs collected");
            let dmidecode_raw = outputs
                .get("dmidecode_bios")
                .map(|s| s.as_str())
                .unwrap_or("");
            if dmidecode_raw.is_empty() {
                warn!("dmidecode bios output empty, falling back to direct parser");
                fallback_bios().await
            } else {
                parse_bios_from_raw(dmidecode_raw)
            }
        }
        Err(err) => {
            warn!(error = %err, "bios command collection failed, falling back");
            fallback_bios().await
        }
    }
}

async fn fallback_cpu() -> Result<ParseOutput<CpuInfo>> {
    Ok(task::spawn_blocking(parse_cpu).await??)
}

async fn fallback_memory() -> Result<Vec<ParseOutput<MemoryInfo>>> {
    Ok(task::spawn_blocking(parse_memory).await??)
}

async fn fallback_bios() -> Result<ParseOutput<BiosInfo>> {
    Ok(task::spawn_blocking(parse_bios).await??)
}

async fn collect_monitor_component(
    config: &CollectConfig,
) -> Result<Vec<ParseOutput<MonitorInfo>>> {
    if !config.monitor.enable {
        return Ok(Vec::new());
    }

    let edids = read_edids_from_sysfs().await?;
    let runner = CmdSource::new(Vec::new(), config.monitor.timeout_ms);
    let xrandr_raw = if config.monitor.enable_xrandr && std::env::var_os("DISPLAY").is_some() {
        match runner
            .run("xrandr", &["--query"], config.monitor.timeout_ms)
            .await
        {
            Ok(out) => Some(out),
            Err(err) => {
                warn!(error = %err, "failed to run xrandr --query");
                None
            }
        }
    } else {
        None
    };

    match parse_monitors_from_raw(&edids, xrandr_raw.as_deref()) {
        Ok(monitors) => Ok(monitors),
        Err(err) => {
            warn!(error = %err, "parse_monitors_from_raw failed, falling back");
            Ok(task::spawn_blocking(parse_monitors).await??)
        }
    }
}

async fn read_edids_from_sysfs() -> Result<HashMap<String, Vec<u8>>> {
    let mut map = HashMap::new();
    let mut dir = match fs::read_dir("/sys/class/drm").await {
        Ok(dir) => dir,
        Err(err) => {
            warn!(error = %err, "failed to read /sys/class/drm");
            return Ok(map);
        }
    };

    while let Ok(Some(entry)) = dir.next_entry().await {
        let name = match entry.file_name().into_string() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if let Some(connector) = normalize_connector_name(&name) {
            let path = entry.path().join("edid");
            match fs::read(&path).await {
                Ok(bytes) if !bytes.is_empty() => {
                    map.insert(connector, bytes);
                }
                Ok(_) => {}
                Err(err) => {
                    warn!(error = %err, path = %path.display(), "failed to read edid");
                }
            }
        }
    }

    Ok(map)
}

fn normalize_connector_name(sys_name: &str) -> Option<String> {
    if !sys_name.starts_with("card") || !sys_name.contains('-') {
        return None;
    }
    let mut parts = sys_name.splitn(2, '-');
    parts.next()?; // skip cardX
    let rest = parts.next()?;
    let pretty = rest
        .replace("-A-", "-")
        .replace("-B-", "-")
        .replace("-C-", "-")
        .replace("-D-", "-");
    Some(pretty)
}

async fn collect_net_component(config: &CollectConfig) -> Result<Vec<ParseOutput<NetInfo>>> {
    if !config.net.enable {
        return Ok(Vec::new());
    }

    let source = CmdSource::new(
        vec![CmdSpec::new(
            "ip_link",
            "ip",
            vec!["-j".into(), "link".into()],
        )],
        config.net.timeout_ms,
    );

    let outputs = match source.collect().await {
        Ok(v) => v,
        Err(err) => {
            warn!(error = %err, "ip -j link failed, falling back to direct parser");
            return Ok(task::spawn_blocking(parse_net).await??);
        }
    };
    let ip_json = outputs.get("ip_link").cloned().unwrap_or_default();
    if ip_json.is_empty() {
        warn!("ip -j link returned empty output, falling back to direct parser");
        return Ok(task::spawn_blocking(parse_net).await??);
    }

    let runner = CmdSource::new(Vec::new(), config.net.timeout_ms);
    let mut ethtool_i_map: HashMap<String, String> = HashMap::new();
    let mut ethtool_map: HashMap<String, String> = HashMap::new();

    if config.net.enable_ethtool {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&ip_json) {
            if let Some(arr) = value.as_array() {
                for item in arr {
                    if let Some(iface) = item.get("ifname").and_then(|v| v.as_str()) {
                        if iface == "lo" {
                            continue;
                        }
                        match runner
                            .run("ethtool", &["-i", iface], config.net.timeout_ms)
                            .await
                        {
                            Ok(out) => {
                                ethtool_i_map.insert(iface.to_string(), out);
                            }
                            Err(err) => {
                                warn!(iface, error = %err, "failed to run ethtool -i");
                            }
                        }
                        match runner.run("ethtool", &[iface], config.net.timeout_ms).await {
                            Ok(out) => {
                                ethtool_map.insert(iface.to_string(), out);
                            }
                            Err(err) => {
                                warn!(iface, error = %err, "failed to run ethtool");
                            }
                        }
                    }
                }
            }
        }
    }

    let lspci = if config.net.enable_lspci {
        match runner.run("lspci", &["-nn"], config.net.timeout_ms).await {
            Ok(out) => Some(out),
            Err(err) => {
                warn!(error = %err, "failed to run lspci -nn");
                None
            }
        }
    } else {
        None
    };

    match parse_net_from_raw(&ip_json, &ethtool_i_map, &ethtool_map, lspci.as_deref()) {
        Ok(nets) => Ok(nets),
        Err(err) => {
            warn!(error = %err, "parse_net_from_raw failed, falling back");
            Ok(task::spawn_blocking(parse_net).await??)
        }
    }
}

async fn collect_storage_component(
    config: &CollectConfig,
) -> Result<Vec<ParseOutput<StorageInfo>>> {
    let commands = vec![CmdSpec::new(
        "lsblk_devices",
        "lsblk",
        vec![
            "-dn".into(),
            "-o".into(),
            "NAME,MODEL,VENDOR,SIZE,ROTA,TRAN,SERIAL,LOG-SEC,PHY-SEC".into(),
        ],
    )];

    let source = CmdSource::new(commands.clone(), config.command_timeout_ms);
    let outputs = match source.collect().await {
        Ok(map) => map,
        Err(err) => {
            warn!(error = %err, "lsblk collection failed, falling back to direct parser");
            return fallback_storage().await;
        }
    };

    let lsblk_raw = outputs.get("lsblk_devices").cloned().unwrap_or_default();
    if lsblk_raw.trim().is_empty() {
        warn!("lsblk output empty, falling back to direct parser");
        return fallback_storage().await;
    }

    // 解析设备名称列表，针对每个设备收集 smartctl/udevadm
    let mut smart_outputs = std::collections::HashMap::new();
    let mut udevadm_outputs = std::collections::HashMap::new();
    for line in lsblk_raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let dev = format!("/dev/{}", parts[0]);

        let smart_cmd = CmdSource::new(
            vec![CmdSpec::new(
                "smartctl_device",
                "smartctl",
                vec!["-H".into(), "-A".into(), dev.clone()],
            )],
            config.command_timeout_ms,
        );
        if let Ok(map) = smart_cmd.collect().await {
            if let Some(output) = map.get("smartctl_device") {
                smart_outputs.insert(dev.clone(), output.clone());
            }
        }

        let udev_cmd = CmdSource::new(
            vec![CmdSpec::new(
                "udevadm_device",
                "udevadm",
                vec![
                    "info".into(),
                    "--query=property".into(),
                    "--name".into(),
                    dev.clone(),
                ],
            )],
            config.command_timeout_ms,
        );
        if let Ok(map) = udev_cmd.collect().await {
            if let Some(output) = map.get("udevadm_device") {
                udevadm_outputs.insert(dev.clone(), output.clone());
            }
        }
    }

    match parse_storage_from_raw(&lsblk_raw, &smart_outputs, &udevadm_outputs) {
        Ok(parsed) => Ok(parsed),
        Err(err) => {
            warn!(error = %err, "parse_storage_from_raw failed, falling back");
            fallback_storage().await
        }
    }
}

async fn fallback_storage() -> Result<Vec<ParseOutput<StorageInfo>>> {
    Ok(task::spawn_blocking(parse_storage).await??)
}
