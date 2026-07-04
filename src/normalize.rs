use hw_model::{Inventory, MemoryInfo, NetInfo, StorageInfo};
use hw_store::component::ComponentRow;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct FormatPrintPayload {
    pub bindid: String,
    pub motherboard: MotherboardSummary,
    pub cpu: Vec<CpuSummary>,
    pub memory: Vec<MemorySummary>,
    pub storage: Vec<StorageSummary>,
    pub network: Vec<NetworkSummary>,
    /// 组件行镜像：保证 JSON 与数据库结构一致
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ComponentRowJson>,
}

#[derive(Debug, Serialize)]
pub struct MotherboardSummary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bios_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sn: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CpuSummary {
    pub id: String,
    pub vendor: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    pub cores: u32,
    pub threads: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_freq_mhz: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_freq_mhz: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct MemorySummary {
    pub slot: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_mb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_mhz: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_number: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StorageSummary {
    pub device: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tran: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bus_info: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NetworkSummary {
    pub iface: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_mbps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "vec_is_empty")]
    pub ipv4: Vec<String>,
    #[serde(skip_serializing_if = "vec_is_empty")]
    pub ipv6: Vec<String>,
}

pub fn build_formatprint_payload(
    inv: &Inventory,
    bind_id: &str,
    sn: Option<&str>,
    component_rows: &[ComponentRow],
) -> FormatPrintPayload {
    let bios = &inv.bios.parsed;
    let motherboard_name = first_non_empty(&[
        bios.board_product_name.as_deref(),
        bios.sys_product_name.as_deref(),
    ])
    .unwrap_or("Unknown Motherboard");

    let motherboard = MotherboardSummary {
        name: motherboard_name.to_string(),
        vendor: first_non_empty(&[
            bios.board_manufacturer.as_deref(),
            bios.sys_manufacturer.as_deref(),
        ])
        .map(str::to_string),
        serial: first_non_empty(&[
            bios.board_serial.as_deref(),
            bios.sys_serial_number.as_deref(),
        ])
        .map(str::to_string),
        uuid: bios.sys_uuid.as_deref().map(str::to_string),
        version: bios.board_version.as_deref().map(str::to_string),
        bios_version: bios.version.as_deref().map(str::to_string),
        sn: sn
            .and_then(|v| {
                let trimmed = v.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .or_else(|| bios.sys_serial_number.as_deref().map(str::to_string)),
    };

    let cpu_info = &inv.cpu.parsed;
    let cpu = vec![CpuSummary {
        id: "CPU0".to_string(),
        vendor: non_empty(cpu_info.vendor.as_str()).unwrap_or_else(|| "Unknown Vendor".into()),
        model: non_empty(cpu_info.name.as_str()).unwrap_or_else(|| "Unknown CPU".into()),
        arch: non_empty(cpu_info.arch.as_str()),
        cores: cpu_info.cores,
        threads: cpu_info.threads,
        max_freq_mhz: cpu_info.max_freq_mhz.or(cpu_info.cur_freq_mhz),
        base_freq_mhz: cpu_info.base_freq_mhz,
    }];

    let memory = inv
        .memory
        .iter()
        .enumerate()
        .map(|(idx, item)| memory_summary(&item.parsed, idx))
        .collect::<Vec<_>>();

    let storage = inv
        .storage
        .iter()
        .map(|item| storage_summary(&item.parsed))
        .collect::<Vec<_>>();

    let network = inv
        .networks
        .iter()
        .map(|item| network_summary(&item.parsed))
        .collect::<Vec<_>>();

    let components = component_rows
        .iter()
        .map(ComponentRowJson::from)
        .collect::<Vec<_>>();

    FormatPrintPayload {
        bindid: bind_id.to_string(),
        motherboard,
        cpu,
        memory,
        storage,
        network,
        components,
    }
}

fn memory_summary(mem: &MemoryInfo, idx: usize) -> MemorySummary {
    MemorySummary {
        slot: memory_slot(mem, idx),
        size_mb: mem.size_mb,
        size: mem.size.as_deref().map(str::to_string),
        speed_mhz: mem.speed_mtps,
        speed: mem.speed.as_deref().map(str::to_string),
        vendor: mem.vendor.as_deref().map(str::to_string),
        serial: mem.serial_number.as_deref().map(str::to_string),
        part_number: mem.part_number.as_deref().map(str::to_string),
    }
}

fn storage_summary(sto: &StorageInfo) -> StorageSummary {
    StorageSummary {
        device: sto.device.clone(),
        model: sto.model.as_deref().map(str::to_string),
        vendor: sto.vendor.as_deref().map(str::to_string),
        size: sto.size.as_deref().map(str::to_string),
        size_bytes: sto.size_bytes,
        media_type: sto.media_type.as_deref().map(str::to_string),
        serial: sto.serial.as_deref().map(str::to_string),
        firmware: sto.firmware.as_deref().map(str::to_string),
        tran: sto.tran.as_deref().map(str::to_string),
        bus_info: sto.bus_info.as_deref().map(str::to_string),
    }
}

fn network_summary(net: &NetInfo) -> NetworkSummary {
    NetworkSummary {
        iface: net.iface.clone(),
        mac: net.mac.as_deref().map(str::to_string),
        driver: net.driver.as_deref().map(str::to_string),
        speed_mbps: net.speed,
        duplex: net.duplex.as_deref().map(str::to_string),
        vendor_id: net.vendor_id.as_deref().map(str::to_string),
        device_id: net.device_id.as_deref().map(str::to_string),
        ipv4: net.ipv4.clone(),
        ipv6: net.ipv6.clone(),
    }
}

fn memory_slot(mem: &MemoryInfo, idx: usize) -> String {
    first_non_empty(&[
        mem.dimm_position.as_deref(),
        mem.locator.as_deref(),
        mem.bank_locator.as_deref(),
    ])
    .map(str::to_string)
    .unwrap_or_else(|| format!("DIMM{idx}"))
}

fn first_non_empty<'a>(candidates: &[Option<&'a str>]) -> Option<&'a str> {
    candidates
        .iter()
        .filter_map(|opt| opt.as_ref().map(|s| s.trim()))
        .find(|val| !val.is_empty())
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn vec_is_empty<T>(v: &Vec<T>) -> bool {
    v.is_empty()
}

#[derive(Debug, Serialize)]
pub struct ComponentRowJson {
    /// 直接复刻数据库字段，便于导入
    #[serde(rename = "fd_CODE", skip_serializing_if = "Option::is_none")]
    pub fd_code: Option<String>,
    #[serde(rename = "fd_NAME", skip_serializing_if = "Option::is_none")]
    pub fd_name: Option<String>,
    #[serde(rename = "fd_SN", skip_serializing_if = "Option::is_none")]
    pub fd_sn: Option<String>,
    #[serde(rename = "fd_TYPE", skip_serializing_if = "Option::is_none")]
    pub fd_type: Option<String>,
    #[serde(rename = "fd_COMPANY", skip_serializing_if = "Option::is_none")]
    pub fd_company: Option<String>,
    #[serde(rename = "fd_VOL", skip_serializing_if = "Option::is_none")]
    pub fd_vol: Option<String>,
    #[serde(rename = "fd_VOL_REAL", skip_serializing_if = "Option::is_none")]
    pub fd_vol_real: Option<String>,
    #[serde(rename = "fd_DEV_TYPE", skip_serializing_if = "Option::is_none")]
    pub fd_dev_type: Option<String>,
    #[serde(rename = "fd_INFO_EX1", skip_serializing_if = "Option::is_none")]
    pub fd_info_ex1: Option<String>,
    #[serde(rename = "fd_INFO_EX2", skip_serializing_if = "Option::is_none")]
    pub fd_info_ex2: Option<String>,
    #[serde(rename = "fd_INFO_EX3", skip_serializing_if = "Option::is_none")]
    pub fd_info_ex3: Option<String>,
    #[serde(rename = "fd_INFO_EX4", skip_serializing_if = "Option::is_none")]
    pub fd_info_ex4: Option<String>,
    #[serde(rename = "fd_INFO_EX5", skip_serializing_if = "Option::is_none")]
    pub fd_info_ex5: Option<String>,
    #[serde(rename = "fd_INFO_EX6", skip_serializing_if = "Option::is_none")]
    pub fd_info_ex6: Option<String>,
    #[serde(rename = "fd_INFO_EX7", skip_serializing_if = "Option::is_none")]
    pub fd_info_ex7: Option<String>,
    #[serde(rename = "fd_INFO_EX8", skip_serializing_if = "Option::is_none")]
    pub fd_info_ex8: Option<String>,
    #[serde(rename = "fd_INFO_EX9", skip_serializing_if = "Option::is_none")]
    pub fd_info_ex9: Option<String>,
    #[serde(rename = "fd_INFO_EX10", skip_serializing_if = "Option::is_none")]
    pub fd_info_ex10: Option<String>,
}

impl From<&ComponentRow> for ComponentRowJson {
    fn from(row: &ComponentRow) -> Self {
        Self {
            fd_code: row.fd_CODE.clone(),
            fd_name: row.fd_NAME.clone(),
            fd_sn: row.fd_SN.clone(),
            fd_type: row.fd_TYPE.clone(),
            fd_company: row.fd_COMPANY.clone(),
            fd_vol: row.fd_VOL.clone(),
            fd_vol_real: row.fd_VOL_REAL.clone(),
            fd_dev_type: row.fd_DEV_TYPE.clone(),
            fd_info_ex1: row.fd_INFO_EX[0].clone(),
            fd_info_ex2: row.fd_INFO_EX[1].clone(),
            fd_info_ex3: row.fd_INFO_EX[2].clone(),
            fd_info_ex4: row.fd_INFO_EX[3].clone(),
            fd_info_ex5: row.fd_INFO_EX[4].clone(),
            fd_info_ex6: row.fd_INFO_EX[5].clone(),
            fd_info_ex7: row.fd_INFO_EX[6].clone(),
            fd_info_ex8: row.fd_INFO_EX[7].clone(),
            fd_info_ex9: row.fd_INFO_EX[8].clone(),
            fd_info_ex10: row.fd_INFO_EX[9].clone(),
        }
    }
}
