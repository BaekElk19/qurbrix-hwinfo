use crate::bindid::build_kv_key;
use crate::component::{CompKey, ComponentRow, ToRow};
use hw_model::{BiosInfo, CpuInfo, GpuInfo, MemoryInfo, MonitorInfo, NetInfo, StorageInfo};

impl CompKey for CpuInfo {
    fn get_composite_key(&self) -> String {
        build_kv_key(&[
            ("vendor", Some(self.vendor.as_str())),
            ("name", Some(self.name.as_str())),
            ("arch", Some(self.arch.as_str())),
            ("family", self.family.as_deref()),
            ("model", self.model.as_deref()),
            ("step", self.stepping.as_deref()),
        ])
    }
}
pub fn system_to_row(cpu: &CpuInfo, bios: &BiosInfo, bind_id: &str) -> ComponentRow {
    let code = bios
        .sys_uuid
        .clone()
        .filter(|uuid| !uuid.trim().is_empty())
        .unwrap_or_else(|| "unknown-system".into());
    let name = bios
        .sys_product_name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| cpu.name.clone());

    let mut row = ComponentRow::new()
        .code(code)
        .name(name)
        .r#type("0")
        .dev_type(&cpu.arch)
        .ex(10, bind_id)
        .ensure_timestamp();

    if let Some(sn) = bios.sys_serial_number.as_deref() {
        if !sn.trim().is_empty() {
            row = row.sn(sn);
        }
    }
    if let Some(company) = bios.sys_manufacturer.as_deref() {
        row = row.company(company);
    }
    if !cpu.name.trim().is_empty() {
        row = row.vol(&cpu.name);
    }
    if let Some(freq) = cpu.cur_freq_mhz.or(cpu.max_freq_mhz).or(cpu.base_freq_mhz) {
        row = row.vol_real(format!("{} MHz", freq));
    }
    row = row.ex(1, cpu.threads.to_string());
    if let Some(cache) = &cpu.cache_l1d {
        row = row.ex(2, cache.as_str());
    }
    if let Some(cache) = &cpu.cache_l1i {
        row = row.ex(3, cache.as_str());
    }
    if let Some(cache) = &cpu.cache_l2 {
        row = row.ex(4, cache.as_str());
    }
    if let Some(cache) = &cpu.cache_l3 {
        row = row.ex(5, cache.as_str());
    }
    if let Some(cache) = &cpu.cache_l4 {
        row = row.ex(6, cache.as_str());
    }

    row
}

impl CompKey for MemoryInfo {
    fn get_composite_key(&self) -> String {
        build_kv_key(&[
            (
                "locator",
                self.dimm_position.as_deref().or(self.locator.as_deref()),
            ),
            ("serial", self.serial_number.as_deref()),
            ("part", self.part_number.as_deref()),
            ("vendor", self.vendor.as_deref()),
            ("size_mb", self.size_mb.map(|v| v.to_string()).as_deref()),
        ])
    }
}
impl ToRow for MemoryInfo {
    fn to_row(&self, bind_id: &str) -> ComponentRow {
        let code = self
            .serial_number
            .clone()
            .or_else(|| self.part_number.clone())
            .or_else(|| self.dimm_position.clone())
            .unwrap_or_else(|| "unknown-dimm".into());

        let name = self
            .part_number
            .clone()
            .or_else(|| self.dimm_position.clone())
            .or_else(|| self.locator.clone())
            .unwrap_or_else(|| "Memory Module".into());

        let mut row = ComponentRow::new()
            .code(code)
            .name(name)
            .r#type("2")
            .ex(10, bind_id);

        if let Some(vendor) = self.vendor.as_deref() {
            row = row.company(vendor);
        }
        if let Some(size) = self.size.as_deref() {
            row = row.vol(size).vol_real(size);
        }
        if let Some(mem_type) = self.r#type.as_deref() {
            row = row.dev_type(mem_type);
        }
        if let Some(locator) = self.dimm_position.as_deref().or(self.locator.as_deref()) {
            row = row.ex(1, locator);
        }
        if let Some(speed) = self.speed.as_deref() {
            row = row.ex(2, speed);
        }
        if let Some(cfg_speed) = self.configured_speed.as_deref() {
            row = row.ex(3, cfg_speed);
        }

        row.ensure_timestamp()
    }
}

impl CompKey for BiosInfo {
    fn get_composite_key(&self) -> String {
        build_kv_key(&[
            ("sys_uuid", self.sys_uuid.as_deref()),
            ("vendor", self.vendor.as_deref()),
            ("version", self.version.as_deref()),
            ("product", self.sys_product_name.as_deref()),
        ])
    }
}
impl ToRow for BiosInfo {
    fn to_row(&self, bind_id: &str) -> ComponentRow {
        let code = self
            .board_serial
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "unknown".into());
        let name = self
            .board_product_name
            .clone()
            .unwrap_or_else(|| "Motherboard".into());

        let mut row = ComponentRow::new()
            .code(code)
            .name(name)
            .r#type("1")
            .ex(10, bind_id);

        if let Some(sn) = self.sys_serial_number.as_deref() {
            row = row.sn(sn);
        }
        if let Some(company) = self.board_manufacturer.as_deref() {
            row = row.company(company);
        }
        if let Some(capacity) = self.mem_max_capacity.as_deref() {
            row = row.vol(capacity);
        }
        if let Some(release) = self.release_date.as_deref() {
            row = row.vol_real(release);
        }
        if let Some(version) = self.version.as_deref() {
            row = row.dev_type(version);
        }
        if let Some(count) = self.mem_number_of_devices {
            row = row.ex(1, count.to_string());
        }
        if let Some(vendor) = self.vendor.as_deref() {
            row = row.ex(2, vendor);
        }

        row.ensure_timestamp()
    }
}

impl CompKey for MonitorInfo {
    fn get_composite_key(&self) -> String {
        build_kv_key(&[
            (
                "connector",
                self.connector.as_deref().or(self.name.as_deref()),
            ),
            ("serial", self.serial.as_deref()),
            ("res", self.resolution.as_deref()),
            ("primary", self.is_primary.map(|b| b.to_string()).as_deref()),
        ])
    }
}
fn format_monitor(info: &MonitorInfo) -> String {
    let vendor = info.vendor.clone().unwrap_or_default();
    let model = info
        .model
        .clone()
        .or_else(|| info.name.clone())
        .unwrap_or_default();
    let serial = info.serial.clone().unwrap_or_default();
    let size = info
        .size_inch
        .map(|inch| format!("{inch:.1}inch"))
        .or_else(|| {
            info.size_mm_w
                .zip(info.size_mm_h)
                .map(|(w, h)| format!("{w}x{h}mm"))
        })
        .unwrap_or_default();
    let current = info.resolution.clone().unwrap_or_default();
    let supported = if info.supported_resolutions.is_empty() {
        String::new()
    } else {
        info.supported_resolutions.join(",")
    };
    [vendor, model, serial, size, current, supported].join("|")
}

pub fn attach_monitors(mut row: ComponentRow, monitors: &[MonitorInfo]) -> ComponentRow {
    for (idx, monitor) in monitors.iter().take(3).enumerate() {
        row = row.ex(4 + idx, format_monitor(monitor));
    }
    row
}

impl CompKey for StorageInfo {
    fn get_composite_key(&self) -> String {
        build_kv_key(&[
            ("wwn", self.wwn.as_deref()),
            ("serial", self.serial.as_deref()),
            ("vendor", self.vendor.as_deref()),
            ("model", self.model.as_deref()),
            ("device", Some(self.device.as_str())),
            ("bus", self.bus_info.as_deref()),
        ])
    }
}
impl ToRow for StorageInfo {
    fn to_row(&self, bind_id: &str) -> ComponentRow {
        let is_ssd = self
            .media_type
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("ssd") || s.eq_ignore_ascii_case("nvme"))
            .unwrap_or(false)
            || self
                .rotation_rate
                .as_deref()
                .map(|s| s.to_ascii_lowercase().contains("solid state"))
                .unwrap_or(false);

        let fd_type = if is_ssd { "4" } else { "3" };
        let code = self
            .serial
            .clone()
            .or_else(|| self.wwn.clone())
            .unwrap_or_else(|| "unknown-disk".into());
        let name = self.model.clone().unwrap_or_else(|| self.device.clone());

        let mut row = ComponentRow::new()
            .code(code)
            .name(name)
            .r#type(fd_type)
            .ex(10, bind_id);

        if let Some(vendor) = self.vendor.as_deref() {
            row = row.company(vendor);
        }
        if let Some(size) = self.size.as_deref() {
            row = row.vol(size).vol_real(size);
        }
        if let Some(version) = self.version.as_deref() {
            row = row.dev_type(version);
        } else if let Some(interface) = self.interface.as_deref() {
            row = row.dev_type(interface);
        }
        if let Some(firmware) = self.firmware.as_deref() {
            row = row.ex(1, firmware);
        }
        if let Some(rotation) = self.rotation_rate.as_deref() {
            row = row.ex(2, rotation);
        }
        if let Some(bus) = self.bus_info.as_deref() {
            row = row.ex(3, bus);
        }

        row.ensure_timestamp()
    }
}

impl CompKey for GpuInfo {
    fn get_composite_key(&self) -> String {
        build_kv_key(&[
            ("vendor", self.vendor.as_deref()),
            ("model", self.model.as_deref().or(self.name.as_deref())),
            ("bus", self.bus_info.as_deref()),
        ])
    }
}
impl ToRow for GpuInfo {
    fn to_row(&self, bind_id: &str) -> ComponentRow {
        let code = self
            .bus_info
            .clone()
            .or_else(|| self.model.clone())
            .unwrap_or_else(|| "gpu".into());
        let name = self
            .model
            .clone()
            .or_else(|| self.name.clone())
            .unwrap_or_else(|| "GPU".into());

        let mut row = ComponentRow::new()
            .code(code)
            .name(name)
            .r#type("5")
            .ex(10, bind_id);

        if let Some(vendor) = self.vendor.as_deref() {
            row = row.company(vendor);
        }
        if let Some(mem_mb) = self.memory_mb {
            row = row
                .vol(format!("{} MB", mem_mb))
                .vol_real(format!("{} MB", mem_mb));
        }
        if let Some(res) = self.cur_resolution.as_deref() {
            row = row.dev_type(res);
        }
        if let Some(min_res) = self.min_resolution.as_deref() {
            row = row.ex(1, min_res);
        }
        if let Some(max_res) = self.max_resolution.as_deref() {
            row = row.ex(2, max_res);
        }
        if let Some(bus) = self.bus_info.as_deref() {
            row = row.ex(3, bus);
        }

        row.ensure_timestamp()
    }
}

impl CompKey for NetInfo {
    fn get_composite_key(&self) -> String {
        build_kv_key(&[
            ("iface", Some(self.iface.as_str())),
            ("mac", self.mac.as_deref()),
            ("pci", self.pci_path.as_deref()),
        ])
    }
}
impl ToRow for NetInfo {
    fn to_row(&self, bind_id: &str) -> ComponentRow {
        let code = self.mac.clone().unwrap_or_else(|| "unknown".into());

        let mut row = ComponentRow::new()
            .code(code)
            .name(&self.iface)
            .r#type("6")
            .ex(10, bind_id);

        if let Some(vendor) = self.vendor_id.as_deref() {
            row = row.company(vendor);
        }
        if let Some(speed) = self.speed {
            row = row.vol(format!("{} Mb/s", speed));
        }
        if let Some(driver) = self.driver.as_deref() {
            row = row.vol_real(driver);
        }
        row = row.dev_type(&self.iface);
        if let Some(device) = self.device_id.as_deref() {
            row = row.ex(1, device);
        }
        if let Some(pci) = self.pci_path.as_deref() {
            row = row.ex(2, pci);
        }
        if let Some(state) = self.operstate.as_deref() {
            row = row.ex(3, state);
        }
        if let Some(mtu) = self.mtu {
            row = row.ex(4, format!("mtu={}", mtu));
        }

        row.ensure_timestamp()
    }
}
