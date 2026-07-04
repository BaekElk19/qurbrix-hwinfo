use anyhow::{anyhow, Context, Result};
use hw_model::{
    BiosInfo, CpuInfo, GpuInfo, MemoryInfo, MonitorInfo, NetInfo, ParseOutput, StorageInfo,
};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct DecodedEdid {
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub size_mm_w: Option<u32>,
    pub size_mm_h: Option<u32>,
    pub manufacture_week: Option<u8>,
    pub manufacture_year: Option<u16>,
}

#[derive(Debug, Clone, Default)]
pub struct XrandrEntry {
    pub connector: String,
    pub is_primary: bool,
    pub current: Option<String>,
    pub supported: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct IpLinkItem {
    iface: String,
    mac: Option<String>,
    mtu: Option<u32>,
    operstate: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct EthtoolInfo {
    driver: Option<String>,
    firmware: Option<String>,
    bus_info: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct PciRecord {
    vendor_id: Option<String>,
    device_id: Option<String>,
    pci_path: Option<String>,
}

/* =============== 公共工具 =============== */

fn run_cmd(cmd: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .with_context(|| format!("failed to spawn command: {} {:?}", cmd, args))?;

    if !out.status.success() {
        return Err(anyhow!(
            "command failed: {} {:?}, stderr={}",
            cmd,
            args,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn kv_map_from_lscpu(s: &str) -> std::collections::HashMap<String, String> {
    let mut m = std::collections::HashMap::new();
    for line in s.lines() {
        if let Some((k, v)) = line.split_once(':') {
            m.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    m
}

fn parse_wxh(s: &str) -> Option<(u32, u32)> {
    // 形如：1920x1080、1920x1080+0+0、"1920x1080i"
    let re = regex::Regex::new(r"(\d{2,5})x(\d{2,5})").ok()?;
    let cap = re.captures(s)?;
    let w = cap.get(1)?.as_str().parse().ok()?;
    let h = cap.get(2)?.as_str().parse().ok()?;
    Some((w, h))
}

fn pick_max_min(res_list: &[String]) -> (Option<String>, Option<String>) {
    let mut parsed: Vec<((u32, u32), &String)> = res_list
        .iter()
        .filter_map(|s| parse_wxh(s).map(|wh| (wh, s)))
        .collect();
    if parsed.is_empty() {
        return (None, None);
    }
    parsed.sort_by_key(|((w, h), _)| w * h);
    let min = parsed.first().map(|(_, s)| (*s).clone());
    let max = parsed.last().map(|(_, s)| (*s).clone());
    (max, min)
}

pub fn decode_edid(bytes: &[u8]) -> DecodedEdid {
    if bytes.len() < 128 {
        return DecodedEdid::default();
    }

    let mut info = DecodedEdid::default();
    info.vendor = edid_mfg_id(bytes);
    info.model =
        edid_descriptor_string(bytes, 0xFC).or_else(|| edid_descriptor_string(bytes, 0xFE));

    // 优先使用描述符中的序列号，兜底使用头部32-bit序列号
    info.serial = edid_descriptor_string(bytes, 0xFF).or_else(|| {
        let raw = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        if raw != 0 {
            Some(format!("{:08X}", raw))
        } else {
            None
        }
    });

    let w_cm = bytes[0x15] as u32;
    let h_cm = bytes[0x16] as u32;
    if w_cm > 0 {
        info.size_mm_w = Some(w_cm * 10);
    }
    if h_cm > 0 {
        info.size_mm_h = Some(h_cm * 10);
    }

    info.manufacture_week = Some(bytes[0x10]);
    info.manufacture_year = Some(1990u16 + bytes[0x11] as u16);

    info
}

pub fn parse_xrandr(raw: &str) -> Vec<XrandrEntry> {
    let mut entries = Vec::new();
    let mut current: Option<XrandrEntry> = None;

    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if line.contains(" connected") || line.contains(" disconnected") {
            if let Some(mut prev) = current.take() {
                prev.supported.sort();
                prev.supported.dedup();
                entries.push(prev);
            }

            let connector = line.split_whitespace().next().unwrap_or("").to_string();
            let is_primary = line.contains(" primary ");
            let mut entry = XrandrEntry {
                connector,
                is_primary,
                current: None,
                supported: Vec::new(),
            };

            if let Some((w, h)) = parse_wxh(line) {
                entry.current = Some(format!("{}x{}", w, h));
            }

            current = Some(entry);
            continue;
        }

        if let Some(cur) = current.as_mut() {
            let trimmed = line.trim_start();
            if trimmed
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
                && trimmed.contains('x')
            {
                if let Some(mode) = trimmed.split_whitespace().next() {
                    let mode = mode.split('@').next().unwrap_or(mode).to_string();
                    if !cur.supported.contains(&mode) {
                        cur.supported.push(mode);
                    }
                }
            }
        }
    }

    if let Some(mut last) = current.take() {
        last.supported.sort();
        last.supported.dedup();
        entries.push(last);
    }

    entries
}

fn parse_ip_link_json(raw: &str) -> Vec<IpLinkItem> {
    let mut out = Vec::new();
    let value: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return out,
    };

    let Some(arr) = value.as_array() else {
        return out;
    };

    for item in arr {
        let iface = item
            .get("ifname")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if iface.is_empty() || iface == "lo" {
            continue;
        }
        let mac = item
            .get("address")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let mtu = item.get("mtu").and_then(|v| v.as_u64()).map(|n| n as u32);
        let operstate = item
            .get("operstate")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        out.push(IpLinkItem {
            iface,
            mac,
            mtu,
            operstate,
        });
    }

    out
}

fn parse_ethtool_i(raw: &str) -> EthtoolInfo {
    let mut info = EthtoolInfo::default();
    for line in raw.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                k if k.contains("driver") => info.driver = Some(value.to_string()),
                k if k.contains("firmware") => info.firmware = Some(value.to_string()),
                k if k.contains("bus-info") => info.bus_info = Some(value.to_string()),
                _ => {}
            }
        }
    }
    info
}

pub fn parse_ethtool(raw: &str) -> (Option<u32>, Option<String>) {
    let mut speed = None;
    let mut duplex = None;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Speed:") {
            if let Some(v) = trimmed.split(':').nth(1) {
                let num = v.trim().trim_end_matches("Mb/s").trim();
                if let Ok(parsed) = num.parse::<u32>() {
                    speed = Some(parsed);
                }
            }
        } else if trimmed.starts_with("Duplex:") {
            duplex = trimmed.split(':').nth(1).map(|v| v.trim().to_string());
        }
    }
    (speed, duplex)
}

fn parse_lspci_ids_for_bus(bus: &str, raw: &str) -> PciRecord {
    let mut rec = PciRecord::default();
    rec.pci_path = Some(bus.to_string());

    for line in raw.lines() {
        if !line.starts_with(bus) {
            continue;
        }
        // 0000:00:19.0 Ethernet controller [0200]: Intel Corporation ... [8086:153a]
        if let Some(idx) = line.find('[') {
            if let Some(end) = line[idx..].find(']') {
                let ids = &line[idx + 1..idx + end];
                if let Some((vendor, device)) = ids.split_once(':') {
                    rec.vendor_id = Some(format!("0x{}", vendor));
                    rec.device_id = Some(format!("0x{}", device));
                    break;
                }
            }
        }
    }

    rec
}
/* ====================== EDID 解析（Monitor 信息） ====================== */

fn edid_mfg_id(edid: &[u8]) -> Option<String> {
    // EDID 8..10：Manufacturer ID（16 bits），5bit 编码：A=1 -> 0x40 偏移
    if edid.len() < 10 {
        return None;
    }
    let id = u16::from_be_bytes([edid[8], edid[9]]);
    let a = ((id >> 10) & 0x1F) as u8;
    let b = ((id >> 5) & 0x1F) as u8;
    let c = (id & 0x1F) as u8;
    let to_char = |v: u8| (b'A' + v - 1) as char;
    if a == 0 || b == 0 || c == 0 {
        return None;
    }
    Some(format!("{}{}{}", to_char(a), to_char(b), to_char(c)))
}

fn edid_descriptor_string(edid: &[u8], tag: u8) -> Option<String> {
    // 四个 18-byte 描述符，从 54 开始，每个 18 字节
    for i in 0..4 {
        let off = 54 + i * 18;
        if edid.len() < off + 18 {
            break;
        }
        let block = &edid[off..off + 18];
        if block[0] == 0x00 && block[1] == 0x00 && block[2] == 0x00 && block[3] == tag {
            // 0xFC Monitor Name；0xFF Serial String
            let text = &block[5..18];
            let s = text
                .iter()
                .map(|&b| {
                    if b >= 0x20 && b <= 0x7E {
                        b as char
                    } else {
                        ' '
                    }
                })
                .collect::<String>()
                .trim()
                .to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

fn sysfs_connectors() -> Vec<(String, String)> {
    // 返回 (sysfs_connector_dir_name, connector_pretty_name)
    // 例如：("card0-HDMI-A-1", "HDMI-1")
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for e in entries.flatten() {
            if let Ok(name) = e.file_name().into_string() {
                if name.contains('-') && name.contains("card") && !name.ends_with("card0") {
                    // 显示输出目录一般为 card0-HDMI-A-1 之类
                    // 把 "card0-" 去掉，把 "-A-" 转换为 "-"
                    let pretty = name
                        .trim_start_matches(|c: char| {
                            c == 'c'
                                || c == 'a'
                                || c == 'r'
                                || c == 'd'
                                || c.is_ascii_digit()
                                || c == '0'
                                || c == '-'
                        })
                        .replace("-A-", "-")
                        .replace("-B-", "-")
                        .replace("-C-", "-");
                    out.push((name, pretty));
                }
            }
        }
    }
    out
}

/* =============== CPU 解析 =============== */

/// 解析 lscpu + /proc/cpuinfo，生成统一的 CpuInfo
pub fn parse_cpu_from_raw(lscpu_raw: &str, cpuinfo_raw: &str) -> Result<ParseOutput<CpuInfo>> {
    parse_cpu_inner(lscpu_raw, cpuinfo_raw)
}

fn parse_cpu_inner(lscpu_raw: &str, cpuinfo_raw: &str) -> Result<ParseOutput<CpuInfo>> {
    let map = kv_map_from_lscpu(lscpu_raw);
    let cpuinfo = cpuinfo_raw;

    // 名称、vendor 与之前相同
    let name = cpuinfo
        .lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.trim().eq_ignore_ascii_case("model name"))
                .map(|(_, v)| v.trim().to_string())
        })
        .or_else(|| map.get("Model name").cloned())
        .unwrap_or_else(|| map.get("Model").cloned().unwrap_or_default());

    let vendor = cpuinfo
        .lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.trim().eq_ignore_ascii_case("vendor_id"))
                .map(|(_, v)| v.trim().to_string())
        })
        .or_else(|| map.get("Vendor ID").cloned())
        .unwrap_or_else(|| "Unknown".into());

    let arch = map
        .get("Architecture")
        .cloned()
        .unwrap_or_else(|| std::env::consts::ARCH.to_string());

    let threads = map
        .get("CPU(s)")
        .and_then(|s| s.split_whitespace().next())
        .and_then(|x| x.parse::<u32>().ok())
        .unwrap_or_else(|| {
            cpuinfo
                .lines()
                .filter(|l| l.to_ascii_lowercase().starts_with("processor"))
                .count() as u32
        });

    let sockets = map
        .get("Socket(s)")
        .and_then(|s| s.split_whitespace().next()?.parse::<u32>().ok());
    let cores_per_socket = map
        .get("Core(s) per socket")
        .and_then(|s| s.split_whitespace().next()?.parse::<u32>().ok());
    let threads_per_core = map
        .get("Thread(s) per core")
        .and_then(|s| s.split_whitespace().next()?.parse::<u32>().ok());

    let cores = match (sockets, cores_per_socket) {
        (Some(s), Some(cps)) if s > 0 && cps > 0 => s.saturating_mul(cps),
        _ => {
            let tpc = threads_per_core.unwrap_or(1).max(1);
            let approx = threads / tpc;
            if approx == 0 {
                threads.max(1)
            } else {
                approx
            }
        }
    };

    let max_freq_mhz = map
        .get("CPU max MHz")
        .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .map(|v| v.round() as u32);

    let base_freq_mhz = map
        .get("CPU base MHz")
        .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .map(|v| v.round() as u32);

    let min_freq_mhz = map
        .get("CPU min MHz")
        .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .map(|v| v.round() as u32);

    let cur_freq_mhz = map
        .get("CPU MHz")
        .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .map(|v| v.round() as u32)
        .or_else(|| {
            let re = Regex::new(r"(?i)cpu mhz\s*:\s*([\d\.]+)").unwrap();
            re.captures(&cpuinfo)
                .and_then(|c| c.get(1)?.as_str().parse::<f64>().ok())
                .map(|v| v.round() as u32)
        });

    // 缓存（含 L4）
    let cache_l1d = map.get("L1d cache").cloned();
    let cache_l1i = map.get("L1i cache").cloned();
    let cache_l2 = map.get("L2 cache").cloned();
    let cache_l3 = map.get("L3 cache").cloned();
    let cache_l4 = map.get("L4 cache").cloned(); // 某些平台存在

    // flags
    let flags = map
        .get("Flags")
        .cloned() // Option<&String> -> Option<String>
        .or_else(|| {
            cpuinfo.lines().find_map(|l| {
                l.split_once(':').and_then(|(k, v)| {
                    if k.trim().eq_ignore_ascii_case("flags") {
                        Some(v.trim().to_string())
                    } else {
                        None
                    }
                })
            })
        })
        .map(|s| {
            s.split_whitespace()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
        });

    // bogomips/family/model/stepping/microcode
    let bogomips = cpuinfo
        .lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.trim().eq_ignore_ascii_case("bogomips"))
                .map(|(_, v)| v.trim().parse::<f64>().ok())
        })
        .flatten();

    let family = cpuinfo.lines().find_map(|l| {
        l.split_once(':')
            .filter(|(k, _)| k.trim().eq_ignore_ascii_case("cpu family"))
            .map(|(_, v)| v.trim().to_string())
    });
    let model = cpuinfo.lines().find_map(|l| {
        l.split_once(':')
            .filter(|(k, _)| k.trim().eq_ignore_ascii_case("model"))
            .map(|(_, v)| v.trim().to_string())
    });
    let stepping = cpuinfo.lines().find_map(|l| {
        l.split_once(':')
            .filter(|(k, _)| k.trim().eq_ignore_ascii_case("stepping"))
            .map(|(_, v)| v.trim().to_string())
    });
    let microcode_version = cpuinfo.lines().find_map(|l| {
        l.split_once(':')
            .filter(|(k, _)| k.trim().eq_ignore_ascii_case("microcode"))
            .map(|(_, v)| v.trim().to_string())
    });

    let address_sizes = map.get("Address sizes").cloned();
    let virtualization = map.get("Virtualization").cloned().or_else(|| {
        if let Some(ref f) = flags {
            if f.iter().any(|x| x == "vmx") {
                return Some("VT-x".into());
            }
            if f.iter().any(|x| x == "svm") {
                return Some("AMD-V".into());
            }
        }
        None
    });

    let parsed = CpuInfo {
        name,
        vendor,
        arch,
        cores,
        threads,
        max_freq_mhz,
        cur_freq_mhz,
        cache_l1i,
        cache_l1d,
        cache_l2,
        cache_l3,
        cache_l4,
        sockets,
        cores_per_socket,
        threads_per_core,
        base_freq_mhz,
        min_freq_mhz,
        bogomips,
        flags,
        family,
        model,
        stepping,
        microcode_version,
        address_sizes,
        virtualization,
    };

    Ok(ParseOutput {
        parsed,
        raw: format!(
            "=== lscpu ===\n{}\n=== /proc/cpuinfo ===\n{}",
            lscpu_raw, cpuinfo_raw
        ),
    })
}

pub fn parse_cpu() -> Result<ParseOutput<CpuInfo>> {
    let lscpu = run_cmd("lscpu", &[]).context("run lscpu")?;
    let cpuinfo = run_cmd("cat", &["/proc/cpuinfo"]).unwrap_or_default();
    parse_cpu_inner(&lscpu, &cpuinfo)
}

/* =============== Memory 解析 =============== */

/// 解析 `dmidecode -t memory`，按“Memory Device”逐条返回（过滤掉未插条位）
pub fn parse_memory_from_raw(
    dmidecode_raw: &str,
    meminfo_raw: Option<&str>,
) -> Result<Vec<ParseOutput<MemoryInfo>>> {
    parse_memory_inner(dmidecode_raw, meminfo_raw)
}

fn parse_memory_inner(
    dmidecode_raw: &str,
    meminfo_raw: Option<&str>,
) -> Result<Vec<ParseOutput<MemoryInfo>>> {
    let blocks: Vec<&str> = dmidecode_raw
        .split("\nHandle ")
        .filter(|b| b.contains("\nMemory Device"))
        .collect();
    let re_kv = Regex::new(r"(?m)^\s*([\w /()\-#]+):\s*(.*)$").unwrap();

    // 系统总内存（便于 size_mb 兜底）
    let meminfo_text = meminfo_raw.unwrap_or("");
    let _total_physical_kb = meminfo_text
        .lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.trim() == "MemTotal")
                .map(|(_, v)| {
                    v.trim()
                        .split_whitespace()
                        .next()
                        .unwrap_or("0")
                        .parse::<u64>()
                        .ok()
                })
        })
        .flatten();

    let mut out = Vec::new();

    for b in blocks {
        let mut map = std::collections::HashMap::<String, String>::new();
        for cap in re_kv.captures_iter(b) {
            let k = cap.get(1).unwrap().as_str().trim().to_string();
            let v = cap.get(2).unwrap().as_str().trim().to_string();
            map.entry(k).or_insert(v);
        }

        let size = map.get("Size").cloned();
        if let Some(sz) = &size {
            if sz.eq_ignore_ascii_case("no module installed")
                || sz.eq_ignore_ascii_case("not installed")
            {
                continue;
            }
        } else {
            continue;
        }

        // 规范化：size_mb、speed_mtps、voltage_mv
        let size_mb = size.as_ref().and_then(|s| {
            // "8 GB" / "16384 MB"
            let low = s.to_ascii_lowercase();
            if low.contains("gb") {
                low.split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<f64>().ok())
                    .map(|g| (g * 1024.0) as u64)
            } else {
                low.split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<u64>().ok())
            }
        });

        let speed_mtps = map
            .get("Speed")
            .and_then(|s| s.split_whitespace().next()?.parse::<u32>().ok());

        let voltage_mv = map
            .get("Configured Voltage")
            .and_then(|s| s.split_whitespace().next())
            .and_then(|x| x.replace("V", "").parse::<f32>().ok())
            .map(|v| (v * 1000.0) as u32);

        // DDR 代际 & ECC 粗略推断
        let ddr_generation = map
            .get("Type")
            .cloned()
            .or_else(|| map.get("Type Detail").cloned());
        let ecc = map.get("Error Correction Type").cloned().or_else(|| {
            map.get("Total Width")
                .zip(map.get("Data Width"))
                .map(|(tw, dw)| {
                    if tw != dw {
                        "ECC".into()
                    } else {
                        "None".into()
                    }
                })
        });

        let parsed = MemoryInfo {
            size,
            vendor: map.get("Manufacturer").cloned(),
            r#type: map.get("Type").cloned(),
            speed: map.get("Speed").cloned(),
            locator: map.get("Locator").cloned(),
            bank_locator: map.get("Bank Locator").cloned(),
            serial_number: map
                .get("Serial Number")
                .cloned()
                .filter(|s| !s.eq_ignore_ascii_case("unknown")),
            part_number: map.get("Part Number").cloned(),
            configured_speed: map.get("Configured Memory Speed").cloned(),
            total_width: map.get("Total Width").cloned(),
            data_width: map.get("Data Width").cloned(),
            configured_voltage: map.get("Configured Voltage").cloned(),
            maximum_voltage: map.get("Maximum Voltage").cloned(),
            minimum_voltage: map.get("Minimum Voltage").cloned(),
            rank: map.get("Rank").cloned(),
            type_detail: map.get("Type Detail").cloned(),

            firmware_version: map.get("Firmware Version").cloned(),
            form_factor: map.get("Form Factor").cloned(),
            asset_tag: map.get("Asset Tag").cloned(),
            manufacture_date: map.get("Manufacture Date").cloned(),
            ecc,
            size_mb,
            speed_mtps,
            voltage_mv,
            ddr_generation,
            dimm_position: map.get("Locator").cloned(),
        };

        out.push(ParseOutput {
            parsed,
            raw: b.to_string(),
        });
    }

    // 可选：把 total_physical_kb 用在需要的地方（汇总层），这里先不写入每条 DIMM
    Ok(out)
}

pub fn parse_memory() -> Result<Vec<ParseOutput<MemoryInfo>>> {
    let raw = run_cmd("dmidecode", &["-t", "memory"]).context("run dmidecode -t memory")?;
    let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    parse_memory_inner(&raw, Some(&meminfo))
}

/* =============== BIOS 解析 =============== */

/// 解析 dmidecode BIOS/System/Board 信息
pub fn parse_bios_from_raw(raw: &str) -> Result<ParseOutput<BiosInfo>> {
    let mut info = BiosInfo {
        vendor: None,
        version: None,
        release_date: None,

        sys_manufacturer: None,
        sys_product_name: None,
        sys_version: None,
        sys_serial_number: None,
        sys_uuid: None,
        sys_wakeup_type: None,
        sys_family: None,

        board_manufacturer: None,
        board_product_name: None,
        board_version: None,
        board_serial: None,
        board_asset_tag: None,
        board_type: None,
        board_features: None,
        board_chassis_handle: None,

        bios_address: None,
        bios_runtime_size: None,
        bios_rom_size: None,
        bios_characteristics: None,
        bios_revision: None,
        firmware_type: None,
        secure_boot: None,

        chassis_manufacturer: None,
        chassis_type: None,
        chassis_version: None,
        chassis_serial: None,
        chassis_oem_info: None,
        chassis_contained_elements: None,

        mem_location: None,
        mem_number_of_devices: None,
        mem_max_capacity: None,
    };

    let mut cur_sec = String::new();
    for line in raw.lines() {
        let t = line.trim_end();
        if t.starts_with("BIOS Information") {
            cur_sec = "bios".into();
            continue;
        }
        if t.starts_with("System Information") {
            cur_sec = "system".into();
            continue;
        }
        if t.starts_with("Base Board Information") {
            cur_sec = "board".into();
            continue;
        }
        if t.starts_with("Chassis Information") {
            cur_sec = "chassis".into();
            continue;
        }
        if t.starts_with("Physical Memory Array") {
            cur_sec = "memarray".into();
            continue;
        }

        if let Some((k, v)) = t.split_once(':') {
            let key = k.trim();
            let val = v.trim().to_string();
            match (cur_sec.as_str(), key) {
                ("bios", "Vendor") => info.vendor = Some(val),
                ("bios", "Version") => info.version = Some(val),
                ("bios", "Release Date") => info.release_date = Some(val),
                ("bios", "Address") => info.bios_address = Some(val),
                ("bios", "Runtime Size") => info.bios_runtime_size = Some(val),
                ("bios", "ROM Size") => info.bios_rom_size = Some(val),
                ("bios", s) if s.starts_with("BIOS Revision") => info.bios_revision = Some(val),
                ("bios", "Characteristics") => info.bios_characteristics = Some(val),

                ("system", "Manufacturer") => info.sys_manufacturer = Some(val),
                ("system", "Product Name") => info.sys_product_name = Some(val),
                ("system", "Version") => info.sys_version = Some(val),
                ("system", "Serial Number") => info.sys_serial_number = Some(val),
                ("system", "UUID") => info.sys_uuid = Some(val),
                ("system", "Wake-up Type") => info.sys_wakeup_type = Some(val),
                ("system", "Family") => info.sys_family = Some(val),

                ("board", "Manufacturer") => info.board_manufacturer = Some(val),
                ("board", "Product Name") => info.board_product_name = Some(val),
                ("board", "Version") => info.board_version = Some(val),
                ("board", "Serial Number") => info.board_serial = Some(val),
                ("board", "Asset Tag") => info.board_asset_tag = Some(val),
                ("board", "Type") => info.board_type = Some(val),
                ("board", "Features") => info.board_features = Some(val),
                ("board", "Chassis Handle") => info.board_chassis_handle = Some(val),

                ("chassis", "Manufacturer") => info.chassis_manufacturer = Some(val),
                ("chassis", "Type") => info.chassis_type = Some(val),
                ("chassis", "Version") => info.chassis_version = Some(val),
                ("chassis", "Serial Number") => info.chassis_serial = Some(val),
                ("chassis", "OEM Information") => info.chassis_oem_info = Some(val),
                ("chassis", "Contained Elements") => {
                    info.chassis_contained_elements = val
                        .split_whitespace()
                        .next()
                        .and_then(|n| n.parse::<i32>().ok())
                }

                ("memarray", "Location") => info.mem_location = Some(val),
                ("memarray", "Use") => { /* 可选 */ }
                ("memarray", "Number Of Devices") => {
                    info.mem_number_of_devices = val
                        .split_whitespace()
                        .next()
                        .and_then(|n| n.parse::<u32>().ok())
                }
                ("memarray", "Maximum Capacity") => info.mem_max_capacity = Some(val),

                _ => {}
            }
        }
    }

    // 固件类型：UEFI/Legacy
    let efi_exists = std::path::Path::new("/sys/firmware/efi").exists();
    info.firmware_type = Some(if efi_exists { "UEFI" } else { "Legacy" }.into());

    // Secure Boot（若可读）
    let sb_path = "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c";
    if efi_exists && std::path::Path::new(sb_path).exists() {
        if let Ok(data) = std::fs::read(sb_path) {
            // 前 4 字节是属性，之后 1 字节是值（0/1）
            if data.len() > 4 {
                info.secure_boot = Some(if data[4] == 1 {
                    "Enabled".into()
                } else {
                    "Disabled".into()
                });
            }
        }
    }

    Ok(ParseOutput {
        parsed: info,
        raw: raw.to_string(),
    })
}

pub fn parse_bios() -> Result<ParseOutput<BiosInfo>> {
    let raw = run_cmd(
        "dmidecode",
        &[
            "-t",
            "bios",
            "-t",
            "system",
            "-t",
            "baseboard",
            "-t",
            "chassis",
            "-t",
            "memory",
        ],
    )
    .context("run dmidecode")?;

    parse_bios_from_raw(&raw)
}

/* =============== Monitor 解析 =============== */

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut buf = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut buf, "{:02X}", b);
    }
    buf
}

fn interface_type_from_name(name: &str) -> Option<String> {
    let lname = name.to_ascii_lowercase();
    if lname.contains("edp") {
        Some("eDP".into())
    } else if lname.contains("hdmi") {
        Some("HDMI".into())
    } else if lname.contains("dp") {
        Some("DP".into())
    } else if lname.contains("vga") {
        Some("VGA".into())
    } else {
        None
    }
}

fn manufacture_to_string(week: Option<u8>, year: Option<u16>) -> Option<String> {
    match (week, year) {
        (Some(week), Some(year)) if week > 0 => {
            let month = ((week as f32 / 4.345).ceil() as u8).clamp(1, 12);
            Some(format!("{:04}-{:02}", year, month))
        }
        (_, Some(year)) => Some(format!("{:04}", year)),
        _ => None,
    }
}

pub fn parse_monitors_from_raw(
    edids: &HashMap<String, Vec<u8>>,
    xrandr_raw: Option<&str>,
) -> Result<Vec<ParseOutput<MonitorInfo>>> {
    let mut results = Vec::new();
    let xr_entries = xrandr_raw.map(parse_xrandr).unwrap_or_default();

    let mut connectors: HashSet<String> = HashSet::new();
    connectors.extend(edids.keys().cloned());
    connectors.extend(xr_entries.iter().map(|e| e.connector.clone()));

    for connector in connectors {
        let edid_bytes = edids.get(&connector);
        let decoded = edid_bytes
            .map(|bytes| decode_edid(bytes))
            .unwrap_or_default();
        let xr = xr_entries.iter().find(|entry| entry.connector == connector);

        let mut supported = xr
            .map(|entry| entry.supported.clone())
            .unwrap_or_else(Vec::new);
        supported.sort();
        supported.dedup();
        let (max_res, min_res) = pick_max_min(&supported);

        let size_inch = match (decoded.size_mm_w, decoded.size_mm_h) {
            (Some(w), Some(h)) if w > 0 && h > 0 => {
                let diag = ((w.pow(2) + h.pow(2)) as f32).sqrt() / 25.4;
                Some((diag * 10.0).round() / 10.0)
            }
            _ => None,
        };

        let production_date =
            manufacture_to_string(decoded.manufacture_week, decoded.manufacture_year);

        let mut raw_blob = String::new();
        if let Some(bytes) = edid_bytes {
            raw_blob.push_str(&format!("EDID:{}:{}\n", connector, bytes_to_hex(bytes)));
        }
        if let Some(entry) = xr {
            raw_blob.push_str(&format!(
                "XRANDR:{}:primary={} current={:?} modes={:?}\n",
                connector, entry.is_primary, entry.current, entry.supported
            ));
        }

        let monitor = MonitorInfo {
            name: Some(connector.clone()),
            vendor: decoded.vendor.clone(),
            model: decoded.model.clone(),
            serial: decoded.serial.clone(),
            connector: Some(connector.clone()),
            interface_type: interface_type_from_name(&connector),
            is_primary: xr.map(|e| e.is_primary),
            resolution: xr.and_then(|e| e.current.clone()),
            max_resolution: max_res,
            min_resolution: min_res,
            supported_resolutions: supported,
            size_mm_w: decoded.size_mm_w,
            size_mm_h: decoded.size_mm_h,
            size_inch,
            production_date,
        };

        results.push(ParseOutput {
            parsed: monitor,
            raw: raw_blob,
        });
    }

    // 稳定排序，确保输出一致
    results.sort_by(|a, b| {
        a.parsed
            .connector
            .as_ref()
            .cmp(&b.parsed.connector.as_ref())
    });

    Ok(results)
}

fn read_edids_from_sysfs() -> HashMap<String, Vec<u8>> {
    let mut map = HashMap::new();
    for (sys_name, pretty) in sysfs_connectors() {
        let path = Path::new("/sys/class/drm").join(sys_name).join("edid");
        if let Ok(bytes) = fs::read(&path) {
            map.insert(pretty.clone(), bytes);
        }
    }
    map
}

pub fn parse_monitors() -> Result<Vec<ParseOutput<MonitorInfo>>> {
    let edids = read_edids_from_sysfs();
    let xrandr_raw = run_cmd("bash", &["-c", "xrandr --verbose 2>/dev/null || true"]).ok();
    parse_monitors_from_raw(&edids, xrandr_raw.as_deref())
}

/* ====================== Storage 解析 ====================== */

pub fn parse_storage_from_raw(
    lsblk_raw: &str,
    smart_outputs: &std::collections::HashMap<String, String>,
    udevadm_outputs: &std::collections::HashMap<String, String>,
) -> Result<Vec<ParseOutput<StorageInfo>>> {
    parse_storage_inner(lsblk_raw, smart_outputs, udevadm_outputs)
}

fn parse_storage_inner(
    lsblk_raw: &str,
    smart_outputs: &std::collections::HashMap<String, String>,
    udevadm_outputs: &std::collections::HashMap<String, String>,
) -> Result<Vec<ParseOutput<StorageInfo>>> {
    let mut result = Vec::new();

    for line in lsblk_raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        // 至少应有 NAME + SIZE
        if parts.len() < 2 {
            continue;
        }

        let name = parts[0];
        let device = format!("/dev/{}", name);
        let model = parts.get(1).map(|s| s.to_string());
        let vendor = parts.get(2).map(|s| s.to_string());
        let size_str = parts.get(3).map(|s| s.to_string());
        let rota = parts.get(4).and_then(|s| s.parse::<u32>().ok()); // 1=HDD, 0=SSD/NVMe
        let tran = parts.get(5).map(|s| s.to_string());
        let serial = parts.get(6).map(|s| s.to_string());
        let log_sec = parts.get(7).and_then(|s| s.parse::<u32>().ok());
        let phy_sec = parts.get(8).and_then(|s| s.parse::<u32>().ok());

        // 计算 size_bytes
        let size_bytes = std::fs::read_to_string(format!("/sys/block/{}/size", name))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .zip(log_sec.map(|ls| ls as u64))
            .map(|(sectors, lb)| sectors.saturating_mul(lb));

        // 介质类型
        let media_type = if name.starts_with("nvme") || tran.as_deref() == Some("nvme") {
            Some("NVMe".into())
        } else if rota == Some(1) {
            Some("HDD".into())
        } else {
            Some("SSD".into())
        };

        let mut firmware = None;
        let mut wwn = None;
        let mut id_model = None;
        let mut id_vendor = None;
        let mut id_path = None;
        let bus_info;
        let mut interface = None;
        let mut guid = None;

        if let Some(props) = udevadm_outputs.get(&device) {
            for l in props.lines() {
                if let Some((k, v)) = l.split_once('=') {
                    match k {
                        "ID_REVISION" => firmware = Some(v.to_string()),
                        "ID_WWN" | "ID_WWN_WITH_EXTENSION" => wwn = Some(v.to_string()),
                        "ID_MODEL" => id_model = Some(v.to_string()),
                        "ID_VENDOR" => id_vendor = Some(v.to_string()),
                        "ID_PATH" => id_path = Some(v.to_string()),
                        "ID_BUS" => interface = Some(v.to_string()), // ata/nvme/usb/scsi...
                        "DM_UUID" | "ID_PART_TABLE_UUID" => guid = Some(v.to_string()),
                        _ => {}
                    }
                }
            }
        }
        bus_info = id_path.clone();

        // I/O 调度、队列深度、TRIM 支持
        let scheduler =
            std::fs::read_to_string(format!("/sys/block/{}/queue/scheduler", name)).ok();
        let queue_depth =
            std::fs::read_to_string(format!("/sys/block/{}/device/queue_depth", name))
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok());
        let discard_max_bytes =
            std::fs::read_to_string(format!("/sys/block/{}/queue/discard_max_bytes", name))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok());
        let trim_supported = Some(discard_max_bytes.unwrap_or(0) > 0);

        // SATA/NVMe 链路速率（尽力而为，不同平台可能缺）
        let sata_link_speed =
            std::fs::read_to_string(format!("/sys/block/{}/device/inquiry", name)).ok();
        let negotiated_link_speed = None::<String>; // 可在 NVMe 用 /sys/class/nvme/nvme*/speed 获取

        // SMART/健康与温度（best-effort）
        let mut smart_status = None;
        let mut temperature = None;
        let mut power_on_hours = None;
        let mut power_cycles = None;

        // 尝试 smartctl
        if let Some(smart) = smart_outputs.get(&device) {
            for l in smart.lines() {
                let ll = l.to_ascii_lowercase();
                if ll.contains("overall-health self-assessment test result") {
                    smart_status = l.split(':').nth(1).map(|v| v.trim().to_string());
                } else if ll.contains("temperature_celsius")
                    || ll.contains("temperature_cel")
                    || ll.contains("current drive temperature")
                {
                    // smartctl 输出格式多样，这里尽力抓一个数字
                    if let Some(num) = l
                        .split_whitespace()
                        .filter_map(|w| w.parse::<i64>().ok())
                        .next()
                    {
                        temperature = Some(format!("{} °C", num));
                    }
                } else if ll.contains("power_on_hours") {
                    power_on_hours = l
                        .split_whitespace()
                        .rev()
                        .find_map(|w| w.parse::<u64>().ok());
                } else if ll.contains("power_cycle_count") || ll.contains("power cycles") {
                    power_cycles = l
                        .split_whitespace()
                        .rev()
                        .find_map(|w| w.parse::<u64>().ok());
                }
            }
        }

        // NVMe 专属：型号/固件/序列（更精确）
        if name.starts_with("nvme") {
            let nvme_root = format!("/sys/block/{}/device", name);
            let nvme_model = std::fs::read_to_string(format!("{}/model", nvme_root))
                .ok()
                .map(|s| s.trim().to_string());
            let nvme_fw = std::fs::read_to_string(format!("{}/firmware_rev", nvme_root))
                .ok()
                .map(|s| s.trim().to_string());
            let nvme_sn = std::fs::read_to_string(format!("{}/serial", nvme_root))
                .ok()
                .map(|s| s.trim().to_string());
            let nvme_wwid = std::fs::read_to_string(format!("{}/wwid", nvme_root))
                .ok()
                .map(|s| s.trim().to_string());

            // 覆盖更准确的信息
            if nvme_model.is_some() { /* 不强制覆盖用户可保留 */ }
            let model = nvme_model.or(model);
            let firmware = nvme_fw.or(firmware);
            let serial = nvme_sn.or(serial);
            let guid2 = nvme_wwid.or(guid);

            // rotationRate：NVMe/SSD 统一为 “Solid State Device”
            let rotation_rate = Some("Solid State Device".into());

            let parsed = StorageInfo {
                device: device.clone(),
                model,
                vendor: id_vendor.or(vendor),
                size: size_str.clone(),
                media_type,
                serial,
                firmware,
                tran: tran.clone(),

                size_bytes,
                realsize: size_str.clone(),
                rotation_rate,
                interface: interface.clone(),
                capabilities: None,
                speed: None,
                version: None,
                description: None,
                ansi_version: None,
                guid: guid2,
                geometry: None,
                bus_info: bus_info.clone(),
                sector_size_logical: log_sec,
                sector_size_physical: phy_sec,
                hardware_class: None,
                device_file: Some(device.clone()),
                device_number: None,
                logical_name: Some(device.clone()),
                physical_id: None,

                wwn,
                smart_status,
                temperature,
                power_on_hours,
                power_cycles,
                scheduler: scheduler.map(|s| s.trim().to_string()),
                queue_depth,
                sata_link_speed,
                negotiated_link_speed,
                trim_supported,
            };
            result.push(ParseOutput {
                parsed,
                raw: line.to_string(),
            });
            continue;
        }

        // rotationRate（HDD/SSD）
        let rotation_rate = if rota == Some(1) {
            // 如需更准确，可从 smartctl -i 抓 "Rotation Rate"
            // 这里若抓不到具体值，先标注"Rotational"
            Some("Rotational".into())
        } else {
            Some("Solid State Device".into())
        };

        let parsed = StorageInfo {
            device: device.clone(),
            model: id_model.or(model),
            vendor: id_vendor.or(vendor),
            size: size_str.clone(),
            media_type,
            serial,
            firmware,
            tran: tran.clone(),

            size_bytes,
            realsize: size_str.clone(),
            rotation_rate,
            interface: interface.clone(),
            capabilities: None,
            speed: None,
            version: None,
            description: None,
            ansi_version: None,
            guid,
            geometry: None,
            bus_info: bus_info.clone(),
            sector_size_logical: log_sec,
            sector_size_physical: phy_sec,
            hardware_class: None,
            device_file: Some(device.clone()),
            device_number: None,
            logical_name: Some(device.clone()),
            physical_id: None,

            wwn,
            smart_status,
            temperature,
            power_on_hours,
            power_cycles,
            scheduler: scheduler.map(|s| s.trim().to_string()),
            queue_depth,
            sata_link_speed,
            negotiated_link_speed,
            trim_supported,
        };

        result.push(ParseOutput {
            parsed,
            raw: line.to_string(),
        });
    }

    Ok(result)
}

pub fn parse_storage() -> Result<Vec<ParseOutput<StorageInfo>>> {
    use anyhow::Context;
    let lsblk = run_cmd(
        "lsblk",
        &[
            "-dn",
            "-o",
            "NAME,MODEL,VENDOR,SIZE,ROTA,TRAN,SERIAL,LOG-SEC,PHY-SEC",
        ],
    )
    .context("run lsblk -dn")?;

    let mut smart_outputs = std::collections::HashMap::new();
    let mut udevadm_outputs = std::collections::HashMap::new();

    for line in lsblk.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let device = format!("/dev/{}", parts[0]);

        if let Ok(smart) = run_cmd("smartctl", &["-H", "-A", &device]) {
            smart_outputs.insert(device.clone(), smart);
        }
        if let Ok(props) = run_cmd("udevadm", &["info", "--query=property", "--name", &device]) {
            udevadm_outputs.insert(device.clone(), props);
        }
    }

    parse_storage_from_raw(&lsblk, &smart_outputs, &udevadm_outputs)
}

/* ====================== GPU 解析 ====================== */

pub fn parse_gpu() -> Result<Vec<ParseOutput<GpuInfo>>> {
    let mut list = Vec::new();
    let mut raw = String::new();

    // 1) lspci -mm -nn -k：拿名称/厂商/型号/版本/驱动/物理ID
    let lspci = run_cmd(
        "bash",
        &[
            "-c",
            "lspci -mm -nn -k 2>/dev/null | grep -A3 -Ei 'VGA|3D' || true",
        ],
    )?;
    raw.push_str(&format!("[lspci -mm -nn -k]\n{lspci}\n"));
    // 一块主显卡即可；多卡按需扩展
    for block in lspci.split("\n\n") {
        if !block.to_lowercase().contains("vga") && !block.to_lowercase().contains("3d") {
            continue;
        }
        // 典型行：00:0f.0 "VGA compatible controller" "VMware" "VMWARE0405" -r00 ...
        let first = block.lines().next().unwrap_or("");
        let bus_info = first
            .split_whitespace()
            .next()
            .map(|s| format!("pci@{}", s));
        // 取 "Vendor" / "Device" 用 -mm 的双引号字段，或从方括号 [vvvv:dddd]
        let re_mm = Regex::new(r#""([^"])""#).unwrap();
        let mut mm = re_mm.captures_iter(first).map(|c| c[1].to_string());
        let _class = mm.next(); // "VGA compatible controller"
        let vendor = mm.next();
        let product = mm.next(); // 可能是型号（VMWARE0405）
                                 // 物理ID（[vvvv:dddd]）
        let phys_id = Regex::new(r"\[([0-9a-fA-F]{4}):([0-9a-fA-F]{4})\]")
            .ok()
            .and_then(|re| re.captures(first).map(|c| format!("[{}:{}]", &c[1], &c[2])));

        // 版本/Revision
        let version = Regex::new(r"-r([0-9a-fA-F]{2})")
            .ok()
            .and_then(|re| re.captures(first).map(|c| c[1].to_string()));

        // Kernel driver in use
        let driver = block
            .lines()
            .find(|l| l.trim_start().starts_with("Kernel driver in use:"))
            .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string());

        // 2) lspci -v -s BUS：拿 IRQ/IOPort/Mem/Capabilities/Description
        let more = if let Some(bus) = first.split_whitespace().next() {
            run_cmd(
                "bash",
                &["-c", &format!("lspci -v -s {} 2>/dev/null || true", bus)],
            )?
        } else {
            String::new()
        };
        raw.push_str(&format!("\n[lspci -v]\n{more}\n"));
        let description = more.lines().next().map(|s| s.trim().to_string());
        let irq = Regex::new(r"IRQ\s(\d)")
            .ok()
            .and_then(|re| re.captures(&more).map(|c| c[1].to_string()));
        let io_port = Regex::new(r"IOPorts?:\s([0-9a-fx\-\(\)=, ])")
            .ok()
            .and_then(|re| re.captures(&more).map(|c| c[1].trim().to_string()));
        // 取所有 Memory at ... 连接起来
        let mem_address = {
            let re = Regex::new(r"Memory at [0-9a-fx]\s*(?:\([^\)]*\))?").unwrap();
            let mut v = vec![];
            for c in re.captures_iter(&more) {
                v.push(c[0].to_string());
            }
            if v.is_empty() {
                None
            } else {
                Some(v.join("  "))
            }
        };
        let capabilities = {
            let mut caps = Vec::new();
            for ln in more.lines() {
                if ln.trim_start().starts_with("Capabilities:") {
                    caps.push(
                        ln.trim()
                            .replacen("Capabilities:", "", 1)
                            .trim()
                            .to_string(),
                    );
                }
            }
            if caps.is_empty() {
                None
            } else {
                Some(caps.join(" | "))
            }
        };

        // 3) xrandr --verbose：补 current/min/max 分辨率
        let xrandr = run_cmd("bash", &["-c", "xrandr --verbose 2>/dev/null || true"])?;
        raw.push_str(&format!("\n[xrandr --verbose]\n{xrandr}\n"));
        let (cur_res, min_res, max_res) = pick_resolutions_from_xrandr(&xrandr);

        // 4) 显存：/sys/class/drm/card*/device/{gpu-info,mem_info_vram_total}
        let memory_mb = detect_vram_mb_sysfs().or_else(|| detect_vram_mb_debugfs());

        let parsed = GpuInfo {
            name: product.clone().or_else(|| description.clone()),
            vendor,
            model: product,
            version,
            driver,
            bus_info,
            io_port,
            mem_address,
            irq,
            capabilities,
            description,
            phys_id,
            module_alias: None,
            width: None,
            memory_mb,
            cur_resolution: cur_res,
            max_resolution: max_res,
            min_resolution: min_res,
        };
        list.push(ParseOutput {
            parsed,
            raw: raw.clone(),
        });
    }

    // 如果没解析到（极端环境），保证返回空 Vec 而不是 Err
    Ok(list)
}

fn pick_resolutions_from_xrandr(xrandr: &str) -> (Option<String>, Option<String>, Option<String>) {
    let re_head =
        Regex::new(r"(?m)^([A-Za-z0-9\-])\sconnected(?:\sprimary)?\s(\dx\d)(?:\\d\\d)?").ok();
    let re_mode = Regex::new(r"(?m)^\s(\dx\d)(?:\s|\t)").ok();
    if let (Some(rh), Some(rm)) = (re_head, re_mode) {
        if let Some(cap) = rh.captures(xrandr) {
            let cur = cap.get(2).map(|m| m.as_str().to_string());
            let head = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            // 收集该输出后续的所有模式
            let mut modes = Vec::<String>::new();
            let mut after = false;
            for ln in xrandr.lines() {
                if !after {
                    after = ln.starts_with(&format!("{head} "));
                    continue;
                }
                if let Some(mm) = rm.captures(ln) {
                    let m = mm[1].to_string();
                    if !modes.contains(&m) {
                        modes.push(m);
                    }
                } else if !ln.starts_with(' ') {
                    break;
                }
            }
            modes.sort();
            let minr = modes.first().cloned();
            let maxr = modes.last().cloned();
            return (cur, minr, maxr);
        }
    }
    (None, None, None)
}

// 读取 /sys/class/drm/card*/device/gpu-info (VRAM total size: 0xXXXXXXXX)
fn detect_vram_mb_sysfs() -> Option<u64> {
    let base = Path::new("/sys/class/drm");
    for ent in fs::read_dir(base).ok()? {
        let ent = ent.ok()?;
        let name = ent.file_name().to_string_lossy().to_string();
        if !name.starts_with("card") {
            continue;
        }
        let p = ent.path().join("device").join("gpu-info");
        if let Ok(s) = fs::read_to_string(&p) {
            for ln in s.lines() {
                if let Some(rest) = ln.strip_prefix("VRAM total size") {
                    // 形如：VRAM total size: 0x12345678
                    let hex = rest
                        .split_whitespace()
                        .last()
                        .unwrap_or("")
                        .trim_start_matches("0x");
                    if let Ok(val) = u64::from_str_radix(hex, 16) {
                        return Some(val / 1024 / 1024);
                    }
                }
            }
        }
    }
    None
}

// 读取 /sys/class/drm/card*/device/mem_info_vram_total 或 debugfs: /sys/kernel/debug/dri/*/amdgpu_vram_mm
fn detect_vram_mb_debugfs() -> Option<u64> {
    // 常见: amdgpu 导出 mem_info_vram_total（字节）
    let base = Path::new("/sys/class/drm");
    for ent in fs::read_dir(base).ok()? {
        let ent = ent.ok()?;
        let name = ent.file_name().to_string_lossy().to_string();
        if !name.starts_with("card") {
            continue;
        }
        let p = ent.path().join("device").join("mem_info_vram_total");
        if let Ok(mut f) = fs::File::open(&p) {
            let mut s = String::new();
            if f.read_to_string(&mut s).is_ok() {
                if let Ok(bytes) = s.trim().parse::<u64>() {
                    return Some(bytes / 1024 / 1024);
                }
            }
        }
    }
    None
}

/* ===================== 网络 ===================== */

fn read_net_sysfs(iface: &str, key: &str) -> Option<String> {
    let path = format!("/sys/class/net/{iface}/{key}");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

pub fn parse_net_from_raw(
    ip_link_json: &str,
    ethtool_i_map: &HashMap<String, String>,
    ethtool_map: &HashMap<String, String>,
    lspci_raw: Option<&str>,
) -> Result<Vec<ParseOutput<NetInfo>>> {
    let mut outputs = Vec::new();
    let links = parse_ip_link_json(ip_link_json);

    for link in links {
        let ethtool_i = ethtool_i_map.get(&link.iface);
        let ethtool = ethtool_map.get(&link.iface);

        let info = ethtool_i
            .map(|raw| parse_ethtool_i(raw))
            .unwrap_or_default();
        let (speed_ethtool, duplex_ethtool) = ethtool
            .map(|raw| parse_ethtool(raw))
            .unwrap_or((None, None));

        let mut speed = speed_ethtool;
        let mut duplex = duplex_ethtool;
        if speed.is_none() {
            speed = read_net_sysfs(&link.iface, "speed").and_then(|s| s.parse::<u32>().ok());
        }
        if duplex.is_none() {
            duplex = read_net_sysfs(&link.iface, "duplex").filter(|s| !s.is_empty());
        }

        let mut pci_record = if let (Some(bus), Some(raw)) = (info.bus_info.as_deref(), lspci_raw) {
            parse_lspci_ids_for_bus(bus, raw)
        } else {
            PciRecord::default()
        };
        if pci_record.pci_path.is_none() {
            pci_record.pci_path = info.bus_info.clone();
        }

        let mut raw_blob = format!(
            "ip-link:{} mac={:?} mtu={:?} operstate={:?}\n",
            link.iface, link.mac, link.mtu, link.operstate
        );
        if let Some(txt) = ethtool_i {
            raw_blob.push_str(&format!("ethtool -i {}:\n{}\n", link.iface, txt));
        }
        if let Some(txt) = ethtool {
            raw_blob.push_str(&format!("ethtool {}:\n{}\n", link.iface, txt));
        }
        if let Some(bus) = &pci_record.pci_path {
            raw_blob.push_str(&format!("pci_path:{}\n", bus));
        }

        let parsed = NetInfo {
            iface: link.iface.clone(),
            mac: link.mac.clone(),
            operstate: link.operstate.clone(),
            mtu: link.mtu,
            speed,
            duplex,
            driver: info.driver.clone(),
            vendor_id: pci_record.vendor_id,
            device_id: pci_record.device_id,
            pci_path: pci_record.pci_path,
            ipv4: Vec::new(),
            ipv6: Vec::new(),
        };

        outputs.push(ParseOutput {
            parsed,
            raw: raw_blob,
        });
    }

    Ok(outputs)
}

pub fn parse_net() -> Result<Vec<ParseOutput<NetInfo>>> {
    let ip_json = run_cmd("ip", &["-j", "link"]).context("run ip -j link")?;

    let mut ethtool_i_map = HashMap::new();
    let mut ethtool_map = HashMap::new();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&ip_json) {
        if let Some(arr) = value.as_array() {
            for item in arr {
                if let Some(iface) = item.get("ifname").and_then(|v| v.as_str()) {
                    if iface == "lo" {
                        continue;
                    }
                    if let Ok(text) = run_cmd("ethtool", &["-i", iface]) {
                        ethtool_i_map.insert(iface.to_string(), text);
                    }
                    if let Ok(text) = run_cmd("ethtool", &[iface]) {
                        ethtool_map.insert(iface.to_string(), text);
                    }
                }
            }
        }
    }

    let lspci = run_cmd("lspci", &["-nn"]).ok();
    parse_net_from_raw(&ip_json, &ethtool_i_map, &ethtool_map, lspci.as_deref())
}
