use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 通用硬件组件接口，提供 JSON 导出、类型编号与组合键
pub trait ComponentInfo {
    fn to_json(&self, fd_sn: Option<&str>) -> Value;
    fn get_type(&self) -> &'static str;
    fn get_composite_key(&self) -> String;
}

/// 统一包装：解析后的设备对象 + 原始命令输出（便于排错/回放）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseOutput<T> {
    pub parsed: T,
    pub raw: String,
}

/// 完整硬件快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub cpu: ParseOutput<CpuInfo>,
    pub memory: Vec<ParseOutput<MemoryInfo>>,
    pub bios: ParseOutput<BiosInfo>,
    pub monitors: Vec<ParseOutput<MonitorInfo>>,
    pub storage: Vec<ParseOutput<StorageInfo>>,
    pub gpus: Vec<ParseOutput<GpuInfo>>,
    pub networks: Vec<ParseOutput<NetInfo>>,
}

impl Inventory {
    pub fn machine_serial(&self) -> Option<&str> {
        self.bios
            .parsed
            .sys_serial_number
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

/* ================= CPU ================== */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// 例如：AMD Ryzen 7 5800H with Radeon Graphics
    pub name: String,
    /// 例如：AuthenticAMD / GenuineIntel / 其他
    pub vendor: String,
    /// x86_64 / aarch64 / loongarch64 ...
    pub arch: String,
    /// 物理核心数（Core(s) per socket * Socket(s)）
    pub cores: u32,
    /// 逻辑线程总数（通常等于 lscpu 的 CPU(s)）
    pub threads: u32,
    /// 最大频率（MHz），读取不到则为 None
    pub max_freq_mhz: Option<u32>,
    /// 当前频率（MHz），读取不到则为 None
    pub cur_freq_mhz: Option<u32>,
    /// L1i/L1d/L2/L3 缓存（原样文本，便于展示/对齐 Deepin）
    pub cache_l1i: Option<String>,
    pub cache_l1d: Option<String>,
    pub cache_l2: Option<String>,
    pub cache_l3: Option<String>,
    // deepin/文档: l4Cache
    pub cache_l4: Option<String>,
    pub sockets: Option<u32>,
    // 文档 coreCount 语义对齐
    pub cores_per_socket: Option<u32>,
    pub threads_per_core: Option<u32>,
    // lscpu "CPU min MHz" / "CPU base MHz"
    pub base_freq_mhz: Option<u32>,
    // lscpu "CPU min MHz"
    pub min_freq_mhz: Option<u32>,
    // /proc/cpuinfo
    pub bogomips: Option<f64>,
    // 指令集标志
    pub flags: Option<Vec<String>>,
    // family/model/stepping
    pub family: Option<String>,
    pub model: Option<String>,
    pub stepping: Option<String>,
    pub microcode_version: Option<String>,
    // e.g. "45 bits physical, 48 bits virtual"
    pub address_sizes: Option<String>,
    // 从 flags 推断或 lscpu "Virtualization"
    pub virtualization: Option<String>,
}

/* ================= Memory ================== */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// dmidecode: “Memory Device” 中的 “Size”（过滤掉 “No Module Installed”）
    pub size: Option<String>,
    pub vendor: Option<String>,
    pub r#type: Option<String>,
    pub speed: Option<String>,
    pub locator: Option<String>,
    pub bank_locator: Option<String>,
    pub serial_number: Option<String>,
    pub part_number: Option<String>,
    pub configured_speed: Option<String>,
    pub total_width: Option<String>,
    pub data_width: Option<String>,
    pub configured_voltage: Option<String>,
    pub maximum_voltage: Option<String>,
    pub minimum_voltage: Option<String>,
    pub rank: Option<String>,
    pub type_detail: Option<String>,
    pub firmware_version: Option<String>, // 文档: firmwareVersion
    pub form_factor: Option<String>,      // DIMM/SODIMM
    pub asset_tag: Option<String>,
    pub manufacture_date: Option<String>,
    pub ecc: Option<String>,     // ECC/None（若仅数组层有，也可放在汇总里）
    pub size_mb: Option<u64>,    // 规范化数值容量
    pub speed_mtps: Option<u32>, // 规范化数值速度
    pub voltage_mv: Option<u32>, // 规范化数值电压
    pub ddr_generation: Option<String>, // DDR3/DDR4/DDR5（从 Type/Detail 推断）
    pub dimm_position: Option<String>, // 规范化槽位
}
/* ================= BIOS ================== */

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BiosInfo {
    /// BIOS 厂商
    pub vendor: Option<String>,
    /// BIOS 版本
    pub version: Option<String>,
    /// BIOS 发布日期
    pub release_date: Option<String>,
    /// 系统厂商（有些场景 BIOS 厂商和系统厂商不同）
    ///pub system_vendor: Option<String>,
    /// 主板产品名
    ///pub product_name: Option<String>,
    /// 主板版本
    ///pub board_version: Option<String>,
    /// 主板序列号
    ///pub board_serial: Option<String>,
    ///  // System
    pub sys_manufacturer: Option<String>,
    pub sys_product_name: Option<String>,
    pub sys_version: Option<String>,
    pub sys_serial_number: Option<String>,
    pub sys_uuid: Option<String>,
    pub sys_wakeup_type: Option<String>,
    pub sys_family: Option<String>,

    // Baseboard
    pub board_manufacturer: Option<String>,
    pub board_product_name: Option<String>,
    pub board_version: Option<String>,
    pub board_serial: Option<String>,
    pub board_asset_tag: Option<String>,
    pub board_type: Option<String>,
    pub board_features: Option<String>,
    pub board_chassis_handle: Option<String>,

    // BIOS extra
    pub bios_address: Option<String>,
    pub bios_runtime_size: Option<String>,
    pub bios_rom_size: Option<String>,
    pub bios_characteristics: Option<String>,
    pub bios_revision: Option<String>,
    pub firmware_type: Option<String>, // UEFI/Legacy
    pub secure_boot: Option<String>,   // Enabled/Disabled（若可检测）

    // Chassis
    pub chassis_manufacturer: Option<String>,
    pub chassis_type: Option<String>,
    pub chassis_version: Option<String>,
    pub chassis_serial: Option<String>,
    pub chassis_oem_info: Option<String>,
    pub chassis_contained_elements: Option<i32>,

    // Physical Memory Array (汇总)
    pub mem_location: Option<String>,
    pub mem_number_of_devices: Option<u32>,
    pub mem_max_capacity: Option<String>,
}
/* ================= Monitor ================== */

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonitorInfo {
    pub name: Option<String>,      // 输出名/标识（eDP-1 / HDMI-1 / Virtual-1）
    pub vendor: Option<String>,    // 厂商（EDID推断）
    pub model: Option<String>,     // 型号（如 “AUO 1234”）
    pub serial: Option<String>,    // 序列号（EDID）
    pub connector: Option<String>, // 同 name
    pub interface_type: Option<String>, // 新增：接口类型（eDP/DP/HDMI/VGA）
    pub is_primary: Option<bool>,  // 新增：是否主屏
    pub resolution: Option<String>, // 当前分辨率（含刷新率：1920x1080@60）
    pub max_resolution: Option<String>, // 最大分辨率
    pub min_resolution: Option<String>, // 最小分辨率
    pub supported_resolutions: Vec<String>, // 新增：模式清单（去重）
    pub size_mm_w: Option<u32>,    // 新增：宽 mm（EDID）
    pub size_mm_h: Option<u32>,    // 新增：高 mm（EDID）
    pub size_inch: Option<f32>,    // 新增：英寸（由 mm 计算）
    pub production_date: Option<String>, // 新增：生产年月（yyyy-MM，EDID/推断）
}

/* ================= Storage ================== */

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorageInfo {
    /// 设备节点（/dev/nvme0n1 /dev/sda ...）
    pub device: String,
    /// 型号（lsblk/udevadm）
    pub model: Option<String>,
    /// 厂商（lsblk/udevadm）
    pub vendor: Option<String>,
    /// 容量（人类可读）
    pub size: Option<String>,
    /// 媒体类型（HDD/SSD/NVMe）
    pub media_type: Option<String>,
    /// 序列号
    pub serial: Option<String>,
    /// 固件版本（udevadm 的 ID_REVISION）
    pub firmware: Option<String>,
    /// 传输协议（SATA/NVMe/USB/...）
    pub tran: Option<String>,
    pub size_bytes: Option<u64>,       // 数值容量
    pub realsize: Option<String>,      // 文档：realsize（与 size 并存）
    pub rotation_rate: Option<String>, // "7200 RPM" / "Solid State Device"
    pub interface: Option<String>,     // "SATA 3.0"/"PCIe 4.0 NVMe"/"USB 3.x"
    pub capabilities: Option<String>,
    pub speed: Option<String>,   // 速率描述
    pub version: Option<String>, // 控制器/驱动版本描述
    pub description: Option<String>,
    pub ansi_version: Option<String>,
    pub guid: Option<String>, // NVMe/EUI-64/WWID
    pub geometry: Option<String>,
    pub bus_info: Option<String>,          // PCI/USB bus addr
    pub sector_size_logical: Option<u32>,  // LOG-SEC
    pub sector_size_physical: Option<u32>, // PHY-SEC
    pub hardware_class: Option<String>,
    pub device_file: Option<String>, // 例如同 device
    pub device_number: Option<String>,
    pub logical_name: Option<String>,
    pub physical_id: Option<String>,

    // SMART / 健康
    pub wwn: Option<String>,
    pub smart_status: Option<String>,
    pub temperature: Option<String>,
    pub power_on_hours: Option<u64>,
    pub power_cycles: Option<u64>,
    pub scheduler: Option<String>,
    pub queue_depth: Option<u32>,
    pub sata_link_speed: Option<String>,
    pub negotiated_link_speed: Option<String>,
    pub trim_supported: Option<bool>,
}

/* ================= GPU ================== */

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GpuInfo {
    pub name: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,       // 新增：型号（如 “VMware VMWARE0405”）
    pub version: Option<String>,     // 新增：版本/Revision
    pub driver: Option<String>,      // 驱动（lspci -k Kernel driver in use）
    pub bus_info: Option<String>,    // 新增：总线（pci@0000:00:0f.0）
    pub io_port: Option<String>,     // 新增：I/O端口（如 2140(size=16)）
    pub mem_address: Option<String>, // 新增：内存地址范围
    pub irq: Option<String>,         // 新增：中断号
    pub capabilities: Option<String>, // 新增：功能(cap_list 等)
    pub description: Option<String>, // 新增：描述（VGA compatible controller）
    pub phys_id: Option<String>,     // 新增：物理ID(含 [vvvv:dddd])
    pub module_alias: Option<String>, // 新增：模块别名
    pub width: Option<String>,       // 新增：位宽
    pub memory_mb: Option<u64>,      // 显存（从 /sys/**/gpu-info 或 mem_info_vram_total 提取）
    pub cur_resolution: Option<String>, // 当前分辨率
    pub max_resolution: Option<String>, // 最大分辨率
    pub min_resolution: Option<String>, // 最小分辨率
}

/* ================= 网络 ================== */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetInfo {
    /// 网卡名（eth0/enp3s0/wlan0）
    pub iface: String,
    /// MAC 地址（/sys/class/net/<iface>/address）
    pub mac: Option<String>,
    /// 运行状态（up/down）
    pub operstate: Option<String>,
    /// MTU
    pub mtu: Option<u32>,
    /// 速率（Mb/s），优先读 /sys/class/net/<iface>/speed，不存在则尝试 ethtool
    pub speed: Option<u32>,
    /// 双工模式（full/half），来自 ethtool
    pub duplex: Option<String>,
    /// 驱动名（来自 ethtool -i）
    pub driver: Option<String>,
    /// PCI vendor/device id
    pub vendor_id: Option<String>,
    pub device_id: Option<String>,
    /// 设备路径（/sys 下的 device 符号链接）
    pub pci_path: Option<String>,
    /// IP 地址（可选）
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
}

fn first_non_empty<'a>(values: &[Option<&'a str>]) -> Option<&'a str> {
    for candidate in values {
        if let Some(v) = candidate {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

impl ComponentInfo for CpuInfo {
    fn to_json(&self, fd_sn: Option<&str>) -> Value {
        let mut val = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Value::Object(ref mut obj) = val {
            if let Some(sn) = fd_sn {
                obj.insert("sn".into(), Value::String(sn.to_string()));
            }
        }
        val
    }

    fn get_type(&self) -> &'static str {
        "7"
    }

    fn get_composite_key(&self) -> String {
        let vendor = self.vendor.trim();
        let name = self.name.trim();
        let arch = self.arch.trim();
        format!(
            "{vendor}|{name}|{arch}|cores={}|threads={}",
            self.cores, self.threads
        )
    }
}

impl ComponentInfo for MemoryInfo {
    fn to_json(&self, fd_sn: Option<&str>) -> Value {
        let mut val = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Value::Object(ref mut obj) = val {
            if let Some(sn) = fd_sn {
                obj.insert("sn".into(), Value::String(sn.to_string()));
            }
        }
        val
    }

    fn get_type(&self) -> &'static str {
        "2"
    }

    fn get_composite_key(&self) -> String {
        let key = first_non_empty(&[
            self.serial_number.as_deref(),
            self.part_number.as_deref(),
            self.locator.as_deref(),
        ])
        .unwrap_or("unknown-mem");
        format!("mem|{key}")
    }
}

impl ComponentInfo for BiosInfo {
    fn to_json(&self, fd_sn: Option<&str>) -> Value {
        let mut val = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Value::Object(ref mut obj) = val {
            if let Some(sn) = fd_sn {
                obj.insert("sn".into(), Value::String(sn.to_string()));
            }
        }
        val
    }

    fn get_type(&self) -> &'static str {
        "1"
    }

    fn get_composite_key(&self) -> String {
        let board_serial = first_non_empty(&[self.board_serial.as_deref()]);
        let product = first_non_empty(&[
            self.board_product_name.as_deref(),
            self.sys_product_name.as_deref(),
        ]);
        let uuid = first_non_empty(&[self.sys_uuid.as_deref()]);
        let serial = board_serial.or(uuid).unwrap_or("unknown-board");
        let prod = product.unwrap_or("unknown-product");
        format!("board|{serial}|{prod}")
    }
}

impl ComponentInfo for StorageInfo {
    fn to_json(&self, fd_sn: Option<&str>) -> Value {
        let mut val = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Value::Object(ref mut obj) = val {
            if let Some(sn) = fd_sn {
                obj.insert("sn".into(), Value::String(sn.to_string()));
            }
        }
        val
    }

    fn get_type(&self) -> &'static str {
        let is_ssd = self
            .media_type
            .as_deref()
            .map(|v| v.eq_ignore_ascii_case("ssd") || v.eq_ignore_ascii_case("nvme"))
            .unwrap_or_else(|| {
                self.rotation_rate
                    .as_deref()
                    .map(|r| r.to_ascii_lowercase().contains("solid state"))
                    .unwrap_or(false)
            });
        if is_ssd {
            "4"
        } else {
            "3"
        }
    }

    fn get_composite_key(&self) -> String {
        let key = first_non_empty(&[
            self.serial.as_deref(),
            self.wwn.as_deref(),
            Some(self.device.as_str()),
        ])
        .unwrap_or("unknown-disk");
        format!("storage|{key}")
    }
}

impl ComponentInfo for GpuInfo {
    fn to_json(&self, fd_sn: Option<&str>) -> Value {
        let mut val = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Value::Object(ref mut obj) = val {
            if let Some(sn) = fd_sn {
                obj.insert("sn".into(), Value::String(sn.to_string()));
            }
        }
        val
    }

    fn get_type(&self) -> &'static str {
        "5"
    }

    fn get_composite_key(&self) -> String {
        let model = first_non_empty(&[self.model.as_deref(), self.name.as_deref()])
            .unwrap_or("unknown-gpu");
        let vendor = first_non_empty(&[self.vendor.as_deref()]).unwrap_or("vendor");
        let bus =
            first_non_empty(&[self.bus_info.as_deref(), self.phys_id.as_deref()]).unwrap_or("bus");
        format!("gpu|{vendor}|{model}|{bus}")
    }
}

impl ComponentInfo for NetInfo {
    fn to_json(&self, fd_sn: Option<&str>) -> Value {
        let mut val = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Value::Object(ref mut obj) = val {
            if let Some(sn) = fd_sn {
                obj.insert("sn".into(), Value::String(sn.to_string()));
            }
        }
        val
    }

    fn get_type(&self) -> &'static str {
        "6"
    }

    fn get_composite_key(&self) -> String {
        let mac = first_non_empty(&[self.mac.as_deref()]).unwrap_or(&self.iface);
        format!("net|{mac}")
    }
}

impl ComponentInfo for MonitorInfo {
    fn to_json(&self, fd_sn: Option<&str>) -> Value {
        let mut val = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Value::Object(ref mut obj) = val {
            if let Some(sn) = fd_sn {
                obj.insert("sn".into(), Value::String(sn.to_string()));
            }
        }
        val
    }

    fn get_type(&self) -> &'static str {
        "8"
    }

    fn get_composite_key(&self) -> String {
        let key = first_non_empty(&[
            self.serial.as_deref(),
            self.connector.as_deref(),
            self.name.as_deref(),
        ])
        .unwrap_or("monitor");
        format!("monitor|{key}")
    }
}
