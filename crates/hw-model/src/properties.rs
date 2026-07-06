use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "kebab-case")]
pub enum DeviceProperties {
    System(SystemDeviceInfo),
    Motherboard(Box<MotherboardInfo>),
    Bios(BiosInfo),
    Cpu(CpuInfo),
    Memory(MemoryInfo),
    Storage(StorageInfo),
    Gpu(GpuInfo),
    Monitor(MonitorInfo),
    Network(NetworkInfo),
    Audio(AudioInfo),
    Bluetooth(BluetoothInfo),
    Input(InputInfo),
    Camera(CameraInfo),
    Battery(BatteryInfo),
    Printer(PrinterInfo),
    Cdrom(CdromInfo),
    Usb(UsbInfo),
    Pci(PciInfo),
    OtherPci(OtherPciInfo),
    OtherDevice(OtherDeviceInfo),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SystemDeviceInfo {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub architecture: Option<String>,
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
    pub version: Option<String>,
    pub serial: Option<String>,
    pub uuid: Option<String>,
    pub wake_up_type: Option<String>,
    pub sku_number: Option<String>,
    pub family: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MotherboardInfo {
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
    pub version: Option<String>,
    pub serial: Option<String>,
    pub asset_tag: Option<String>,
    pub location_in_chassis: Option<String>,
    pub chassis_handle: Option<String>,
    pub chassis_manufacturer: Option<String>,
    pub chassis_type: Option<String>,
    pub chassis_version: Option<String>,
    pub chassis_serial: Option<String>,
    pub chassis_asset_tag: Option<String>,
    pub chassis_boot_up_state: Option<String>,
    pub chassis_power_supply_state: Option<String>,
    pub chassis_thermal_state: Option<String>,
    pub chassis_security_status: Option<String>,
    pub chassis_oem_information: Option<String>,
    pub chassis_height: Option<String>,
    pub chassis_power_cords: Option<String>,
    pub chassis_contained_elements: Option<String>,
    pub chassis_sku_number: Option<String>,
    pub memory_array_location: Option<String>,
    pub memory_array_use: Option<String>,
    pub memory_array_error_correction_type: Option<String>,
    pub memory_array_maximum_capacity: Option<String>,
    pub memory_array_error_information_handle: Option<String>,
    pub memory_array_number_of_devices: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BiosInfo {
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub release_date: Option<String>,
    pub firmware_type: Option<String>,
    pub secure_boot: Option<String>,
    pub language_description_format: Option<String>,
    pub installable_languages: Vec<String>,
    pub currently_installed_language: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CpuInfo {
    pub name: Option<String>,
    pub vendor: Option<String>,
    pub architecture: Option<String>,
    pub cores: Option<u32>,
    pub threads: Option<u32>,
    pub sockets: Option<u32>,
    pub max_freq_mhz: Option<u32>,
    pub min_freq_mhz: Option<u32>,
    pub current_freq_mhz: Option<u32>,
    pub family: Option<String>,
    pub model: Option<String>,
    pub stepping: Option<String>,
    pub bogomips: Option<String>,
    pub virtualization: Option<String>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemoryInfo {
    pub size_bytes: Option<u64>,
    pub vendor: Option<String>,
    pub memory_type: Option<String>,
    pub speed_mtps: Option<u32>,
    pub total_width_bits: Option<u32>,
    pub data_width_bits: Option<u32>,
    pub min_voltage_v: Option<f32>,
    pub max_voltage_v: Option<f32>,
    pub configured_voltage_v: Option<f32>,
    pub locator: Option<String>,
    pub serial: Option<String>,
    pub part_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StorageInfo {
    pub device_node: Option<String>,
    pub size_bytes: Option<u64>,
    pub media_type: Option<String>,
    pub controller_vendor: Option<String>,
    pub controller_model: Option<String>,
    pub controller_driver: Option<String>,
    pub firmware: Option<String>,
    pub wwn: Option<String>,
    pub smart_status: Option<String>,
    pub temperature_celsius: Option<f32>,
    pub power_on_hours: Option<u64>,
    pub power_cycle_count: Option<u64>,
    pub available_spare_percent: Option<u8>,
    pub available_spare_threshold_percent: Option<u8>,
    pub percentage_used: Option<u8>,
    pub data_units_read: Option<u64>,
    pub data_units_written: Option<u64>,
    pub media_errors: Option<u64>,
    pub error_log_entries: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GpuInfo {
    pub vendor: Option<String>,
    pub renderer: Option<String>,
    pub opengl_vendor: Option<String>,
    pub opengl_version: Option<String>,
    pub memory_bytes: Option<u64>,
    pub current_resolution: Option<String>,
    pub max_resolution: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MonitorInfo {
    pub connector: Option<String>,
    pub resolution: Option<String>,
    pub max_resolution: Option<String>,
    pub size_mm: Option<(u32, u32)>,
    pub production_date: Option<String>,
    pub manufacturer: Option<String>,
    pub manufacturer_name: Option<String>,
    pub product: Option<String>,
    pub product_code: Option<u16>,
    pub serial: Option<String>,
    pub manufactured_year: Option<u16>,
    pub manufactured_week: Option<u8>,
    pub size_cm: Option<(u8, u8)>,
    pub diagonal_inches: Option<f32>,
    pub gamma: Option<f32>,
    pub preferred_width: Option<u16>,
    pub preferred_height: Option<u16>,
    pub preferred_refresh_hz: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NetworkInfo {
    pub interface: Option<String>,
    pub network_type: Option<String>,
    pub mac: Option<String>,
    pub operstate: Option<String>,
    pub speed_mbps: Option<u32>,
    pub duplex: Option<String>,
    pub firmware: Option<String>,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AudioInfo {
    pub card_index: Option<u32>,
    pub card_name: Option<String>,
    pub codec: Option<String>,
    pub subsystem: Option<String>,
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BluetoothInfo {
    pub address: Option<String>,
    pub controller_name: Option<String>,
    pub powered: Option<bool>,
    pub discoverable: Option<bool>,
    pub paired_device_count: Option<u32>,
    pub paired_devices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InputInfo {
    pub input_kind: InputKind,
    pub event_node: Option<String>,
    pub phys: Option<String>,
    pub uniq: Option<String>,
    pub handlers: Vec<String>,
    pub bus_type: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum InputKind {
    Keyboard,
    Mouse,
    Touchpad,
    Touchscreen,
    Tablet,
    #[default]
    UnknownInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CameraInfo {
    pub video_node: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BatteryInfo {
    pub power_type: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub technology: Option<String>,
    pub state: Option<String>,
    pub capacity_percent: Option<f32>,
    pub energy_full_wh: Option<f32>,
    pub energy_design_wh: Option<f32>,
    pub energy_now_wh: Option<f32>,
    pub voltage_v: Option<f32>,
    pub temperature_celsius: Option<f32>,
    pub cycle_count: Option<u32>,
    pub present: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PrinterInfo {
    pub queue_name: Option<String>,
    pub accepting: Option<bool>,
    pub device_uri: Option<String>,
    pub make_model: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CdromInfo {
    pub device_node: Option<String>,
    pub media_present: Option<bool>,
    pub firmware: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UsbInfo {
    pub bus_number: Option<String>,
    pub device_number: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub class: Option<String>,
    pub subclass: Option<String>,
    pub protocol: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub speed: Option<String>,
    pub max_power_ma: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PciInfo {
    pub address: String,
    pub class_name: Option<String>,
    pub class_id: Option<String>,
    pub vendor: Option<String>,
    pub vendor_id: Option<String>,
    pub device: Option<String>,
    pub device_id: Option<String>,
    pub subsystem_vendor_id: Option<String>,
    pub subsystem_device_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OtherPciInfo {
    pub original_class: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OtherDeviceInfo {
    pub original_kind: Option<String>,
    pub reason: String,
}
