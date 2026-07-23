use async_trait::async_trait;
use hw_inventory::{canonicalize_devices, quick_probe_with_runner, QuickProbeConfig};
use hw_model::{
    BusInfo, CoreIdentityGroup, CpuInfo, Device, DeviceKind, DeviceProperties, DriverInfo,
    DriverStatus, GpuInfo, MemoryInfo, NetworkInfo, StorageInfo, SystemDeviceInfo,
};
use hw_source::{
    CommandSpec, FakeSourceRunner, GlobResult, SourceBytesResult, SourceResult, SourceRunner,
};
use std::{
    collections::BTreeSet,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

fn complete_devices() -> Vec<Device> {
    let system = Device::new(
        "system:fixture",
        DeviceKind::System,
        "Fixture",
        DeviceProperties::System(SystemDeviceInfo {
            uuid: Some("ABC-123".into()),
            serial: Some("SYS-1".into()),
            manufacturer: Some("Example".into()),
            product_name: Some("Workstation".into()),
            kernel: Some("6.6.1".into()),
            os: Some("Example Linux".into()),
            architecture: Some("x86_64".into()),
            ..SystemDeviceInfo::default()
        }),
    );
    let cpu = Device::new(
        "cpu:0",
        DeviceKind::Cpu,
        "Example CPU",
        DeviceProperties::Cpu(Box::new(CpuInfo {
            vendor: Some("Example".into()),
            name: Some("X1".into()),
            architecture: Some("x86_64".into()),
            cores: Some(8),
            threads: Some(16),
            sockets: Some(1),
            current_freq_mhz: Some(2100),
            ..CpuInfo::default()
        })),
    );
    let memory = Device::new(
        "memory:0",
        DeviceKind::Memory,
        "DIMM 0",
        DeviceProperties::Memory(MemoryInfo {
            serial: Some("DIMM-1".into()),
            part_number: Some("DDR5-X".into()),
            locator: Some("A0".into()),
            size_bytes: Some(16 * 1024 * 1024 * 1024),
            firmware_version: Some("1.0".into()),
            ..MemoryInfo::default()
        }),
    );
    let mut storage = Device::new(
        "storage:0",
        DeviceKind::Storage,
        "Fixture SSD",
        DeviceProperties::Storage(StorageInfo {
            wwn: Some("WWN-1".into()),
            firmware: Some("FW-1".into()),
            temperature_celsius: Some(34.0),
            power_on_hours: Some(10),
            ..StorageInfo::default()
        }),
    );
    storage.serial = Some("SSD-1".into());
    storage.model = Some("FastDisk".into());
    storage.bus = Some(BusInfo::Pci {
        address: "0000:01:00.0".into(),
        vendor_id: Some("1234".into()),
        device_id: Some("5678".into()),
        subsystem_vendor_id: None,
        subsystem_device_id: None,
        class: None,
    });
    storage.driver = Some(DriverInfo {
        name: Some("nvme".into()),
        version: Some("1.0".into()),
        modules: vec!["nvme".into()],
        provider: None,
        status: DriverStatus::InUse,
    });
    let network = Device::new(
        "network:eth0",
        DeviceKind::Network,
        "Ethernet",
        DeviceProperties::Network(NetworkInfo {
            interface: Some("eth0".into()),
            mac: Some("00:11:22:33:44:55".into()),
            operstate: Some("up".into()),
            speed_mbps: Some(1000),
            ipv4: vec!["192.0.2.2".into()],
            firmware: Some("NIC-FW-1".into()),
            driver_version: Some("2.0".into()),
            ..NetworkInfo::default()
        }),
    );
    let gpu = Device::new(
        "gpu:0",
        DeviceKind::Gpu,
        "Example GPU",
        DeviceProperties::Gpu(GpuInfo {
            vendor: Some("Example".into()),
            description: Some("G1".into()),
            renderer: Some("G1 Hardware".into()),
            clock_mhz: Some(900),
            ..GpuInfo::default()
        }),
    )
    .with_bus(BusInfo::Pci {
        address: "0000:02:00.0".into(),
        vendor_id: Some("abcd".into()),
        device_id: Some("0123".into()),
        subsystem_vendor_id: Some("abcd".into()),
        subsystem_device_id: Some("0001".into()),
        class: Some("0300".into()),
    });
    vec![system, cpu, memory, storage, network, gpu]
}

fn canonical(devices: &[Device]) -> hw_model::QuickProbeReport {
    canonicalize_devices(
        devices,
        Vec::new(),
        BTreeSet::new(),
        "2026-07-23T00:00:00Z".into(),
    )
    .unwrap()
}

#[test]
fn canonicalization_is_order_whitespace_case_and_duplicate_stable() {
    let devices = complete_devices();
    let expected = canonical(&devices);
    let mut reordered = devices.clone();
    reordered.reverse();
    reordered.push(reordered[0].clone());
    if let DeviceProperties::System(info) = &mut reordered
        .iter_mut()
        .find(|device| device.kind == DeviceKind::System)
        .unwrap()
        .properties
    {
        info.uuid = Some("  abc-123  ".into());
        info.manufacturer = Some("EXAMPLE".into());
    }
    let actual = canonical(&reordered);
    assert_eq!(actual.machine_bind_id, expected.machine_bind_id);
    assert_eq!(
        actual.configuration_fingerprint,
        expected.configuration_fingerprint
    );
    assert_eq!(actual.identity_records, expected.identity_records);
}

#[test]
fn physical_component_change_changes_machine_identity() {
    let devices = complete_devices();
    let expected = canonical(&devices);
    let mut changed = devices;
    let network = changed
        .iter_mut()
        .find(|device| device.kind == DeviceKind::Network)
        .unwrap();
    if let DeviceProperties::Network(info) = &mut network.properties {
        info.mac = Some("00:11:22:33:44:66".into());
    }
    let changed = canonical(&changed);
    assert_ne!(changed.machine_bind_id, expected.machine_bind_id);
    assert_ne!(
        changed.configuration_fingerprint,
        expected.configuration_fingerprint
    );
}

#[test]
fn firmware_kernel_and_driver_change_only_configuration() {
    let devices = complete_devices();
    let expected = canonical(&devices);
    let mut changed = devices;
    for device in &mut changed {
        match &mut device.properties {
            DeviceProperties::System(info) => info.kernel = Some("6.7.0".into()),
            DeviceProperties::Storage(info) => info.firmware = Some("FW-2".into()),
            _ => {}
        }
        if let Some(driver) = &mut device.driver {
            driver.version = Some("2.0".into());
        }
    }
    let changed = canonical(&changed);
    assert_eq!(changed.machine_bind_id, expected.machine_bind_id);
    assert_ne!(
        changed.configuration_fingerprint,
        expected.configuration_fingerprint
    );
}

#[test]
fn hot_and_network_runtime_fields_do_not_change_either_fingerprint() {
    let devices = complete_devices();
    let expected = canonical(&devices);
    let mut changed = devices;
    for device in &mut changed {
        match &mut device.properties {
            DeviceProperties::Cpu(info) => info.current_freq_mhz = Some(4900),
            DeviceProperties::Storage(info) => {
                info.temperature_celsius = Some(70.0);
                info.power_on_hours = Some(999);
            }
            DeviceProperties::Network(info) => {
                info.operstate = Some("down".into());
                info.speed_mbps = Some(100);
                info.ipv4 = vec!["198.51.100.9".into()];
            }
            DeviceProperties::Gpu(info) => info.clock_mhz = Some(1500),
            _ => {}
        }
    }
    let changed = canonical(&changed);
    assert_eq!(changed.machine_bind_id, expected.machine_bind_id);
    assert_eq!(
        changed.configuration_fingerprint,
        expected.configuration_fingerprint
    );
}

#[test]
fn placeholders_virtual_network_and_software_gpu_are_excluded() {
    let mut devices = complete_devices();
    if let DeviceProperties::System(info) = &mut devices[0].properties {
        info.serial = Some("To Be Filled By O.E.M.".into());
    }
    let mut virtual_network = devices
        .iter()
        .find(|device| device.kind == DeviceKind::Network)
        .unwrap()
        .clone();
    virtual_network.id = "network:veth0".into();
    virtual_network.bus = Some(BusInfo::Virtual);
    if let DeviceProperties::Network(info) = &mut virtual_network.properties {
        info.interface = Some("veth0".into());
        info.mac = Some("de:ad:be:ef:00:01".into());
    }
    devices.push(virtual_network);
    let mut random_mac_network = devices
        .iter()
        .find(|device| device.kind == DeviceKind::Network)
        .unwrap()
        .clone();
    random_mac_network.id = "network:wlan0".into();
    if let DeviceProperties::Network(info) = &mut random_mac_network.properties {
        info.interface = Some("wlan0".into());
        info.mac = Some("02:11:22:33:44:55".into());
    }
    devices.push(random_mac_network);
    let mut software_gpu = devices
        .iter()
        .find(|device| device.kind == DeviceKind::Gpu)
        .unwrap()
        .clone();
    software_gpu.id = "gpu:software".into();
    software_gpu.name = "llvmpipe".into();
    if let DeviceProperties::Gpu(info) = &mut software_gpu.properties {
        info.renderer = Some("LLVMpipe software rasterizer".into());
    }
    devices.push(software_gpu);
    let report = canonical(&devices);
    assert_eq!(
        report
            .identity_records
            .iter()
            .filter(|record| record.starts_with("physical_network:"))
            .count(),
        1
    );
    assert_eq!(
        report
            .identity_records
            .iter()
            .filter(|record| record.starts_with("gpu:"))
            .count(),
        1
    );
    assert!(!report
        .identity_records
        .iter()
        .any(|record| record.contains("filled")));
}

#[test]
fn trusted_absence_is_distinct_from_missing_enumeration() {
    let mut devices = complete_devices();
    devices.retain(|device| device.kind != DeviceKind::Network);
    let missing = canonical(&devices);
    assert!(!missing.coverage.core_complete());
    assert!(missing
        .coverage
        .missing
        .contains(&CoreIdentityGroup::PhysicalNetwork));

    let trusted = canonicalize_devices(
        &devices,
        Vec::new(),
        BTreeSet::from([CoreIdentityGroup::PhysicalNetwork]),
        "2026-07-23T00:00:00Z".into(),
    )
    .unwrap();
    assert!(trusted.coverage.core_complete());
    assert!(trusted
        .identity_records
        .contains(&"absence:group=physical_network".to_string()));
}

struct TrackingRunner {
    inner: FakeSourceRunner,
    active: AtomicUsize,
}

#[async_trait]
impl SourceRunner for TrackingRunner {
    async fn run_command(&self, command: &CommandSpec, timeout: Duration) -> SourceResult {
        self.active.fetch_add(1, Ordering::SeqCst);
        let result = self.inner.run_command(command, timeout).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        result
    }

    async fn read_file(&self, path: &Path) -> SourceResult {
        self.active.fetch_add(1, Ordering::SeqCst);
        let result = self.inner.read_file(path).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        result
    }

    async fn read_file_bytes(&self, path: &Path) -> SourceBytesResult {
        self.active.fetch_add(1, Ordering::SeqCst);
        let result = self.inner.read_file_bytes(path).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        result
    }

    async fn canonicalize_path(&self, path: &Path) -> SourceResult {
        self.active.fetch_add(1, Ordering::SeqCst);
        let result = self.inner.canonicalize_path(path).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        result
    }

    async fn glob(&self, pattern: &str) -> GlobResult {
        self.active.fetch_add(1, Ordering::SeqCst);
        let result = self.inner.glob(pattern).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        result
    }
}

#[tokio::test]
async fn source_failures_are_reported_and_no_source_call_survives_return() {
    let runner = TrackingRunner {
        inner: FakeSourceRunner::new(),
        active: AtomicUsize::new(0),
    };
    let report = quick_probe_with_runner(
        &runner,
        QuickProbeConfig {
            timeout: Duration::from_millis(1),
        },
    )
    .await
    .unwrap();
    assert!(!report.coverage.core_complete());
    assert!(!report.warnings.is_empty());
    assert_eq!(runner.active.load(Ordering::SeqCst), 0);
}
