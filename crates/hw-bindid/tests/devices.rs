use hw_bindid::devices::component_keys_from_devices;
use hw_model::{
    Device, DeviceKind, DeviceProperties, GpuInfo, MemoryInfo, MonitorInfo, MotherboardInfo,
    NetworkInfo, StorageInfo, SystemDeviceInfo,
};

#[test]
fn builds_sorted_component_keys_for_supported_device_kinds() {
    let mut storage = Device::new(
        "storage0",
        DeviceKind::Storage,
        "Boot Disk",
        DeviceProperties::Storage(StorageInfo {
            controller_model: Some("Fallback Controller".to_string()),
            ..StorageInfo::default()
        }),
    );
    storage.serial = Some("DISK123".to_string());
    storage.model = Some("Fast Disk".to_string());

    let mut gpu = Device::new(
        "gpu0",
        DeviceKind::Gpu,
        "RTX 4090",
        DeviceProperties::Gpu(GpuInfo {
            description: Some("Ignored Fallback".to_string()),
            ..GpuInfo::default()
        }),
    );
    gpu.model = Some("AD102".to_string());

    let devices = vec![
        Device::new(
            "network0",
            DeviceKind::Network,
            "Ethernet",
            DeviceProperties::Network(NetworkInfo {
                mac: Some("AA:BB:CC:DD:EE:FF".to_string()),
                ..NetworkInfo::default()
            }),
        ),
        storage,
        Device::new(
            "system0",
            DeviceKind::System,
            "Host",
            DeviceProperties::System(SystemDeviceInfo {
                manufacturer: Some("GEIT".to_string()),
                product_name: Some("UT6619-FC2".to_string()),
                ..SystemDeviceInfo::default()
            }),
        ),
        Device::new(
            "memory0",
            DeviceKind::Memory,
            "DIMM 0",
            DeviceProperties::Memory(MemoryInfo {
                serial: Some("RAM123".to_string()),
                part_number: Some("PN-1".to_string()),
                ..MemoryInfo::default()
            }),
        ),
        Device::new(
            "board0",
            DeviceKind::Motherboard,
            "Mainboard",
            DeviceProperties::Motherboard(Box::new(MotherboardInfo {
                serial: Some("BOARD123".to_string()),
                product_name: Some("X670E".to_string()),
                ..MotherboardInfo::default()
            })),
        ),
        gpu,
    ];

    assert_eq!(
        component_keys_from_devices(&devices),
        vec![
            "gpu:model=AD102|name=RTX 4090".to_string(),
            "memory:product=PN-1|serial=RAM123".to_string(),
            "motherboard:product=X670E|serial=BOARD123".to_string(),
            "network:mac=aa:bb:cc:dd:ee:ff".to_string(),
            "storage:model=Fast Disk|serial=DISK123".to_string(),
            "system:manufacturer=GEIT|product=UT6619-FC2".to_string(),
        ]
    );
}

#[test]
fn network_keys_use_only_mac_and_normalize_to_lowercase() {
    let device = Device::new(
        "network0",
        DeviceKind::Network,
        "USB Ethernet",
        DeviceProperties::Network(NetworkInfo {
            interface: Some("eth0".to_string()),
            network_type: Some("ethernet".to_string()),
            mac: Some("AA:BB:CC:DD:EE:FF".to_string()),
            operstate: Some("up".to_string()),
            speed_mbps: Some(1000),
            duplex: Some("full".to_string()),
            firmware: Some("1.0.0".to_string()),
            ipv4: vec!["192.168.1.10".to_string()],
            ipv6: vec!["fe80::1".to_string()],
        }),
    );

    assert_eq!(
        component_keys_from_devices(&[device]),
        vec!["network:mac=aa:bb:cc:dd:ee:ff".to_string()]
    );
}

#[test]
fn ignores_cpu_and_monitor_devices() {
    let devices = vec![
        Device::new(
            "cpu0",
            DeviceKind::Cpu,
            "CPU",
            DeviceProperties::Cpu(Box::default()),
        ),
        Device::new(
            "monitor0",
            DeviceKind::Monitor,
            "Display",
            DeviceProperties::Monitor(MonitorInfo {
                manufacturer: Some("Dell".to_string()),
                product: Some("U2720Q".to_string()),
                ..MonitorInfo::default()
            }),
        ),
    ];

    assert!(component_keys_from_devices(&devices).is_empty());
}

#[test]
fn ignores_loopback_and_all_zero_mac_network_devices() {
    let devices = vec![
        Device::new(
            "network0",
            DeviceKind::Network,
            "Loopback Interface",
            DeviceProperties::Network(NetworkInfo {
                interface: Some("lo".to_string()),
                mac: Some("AA:BB:CC:DD:EE:FF".to_string()),
                ..NetworkInfo::default()
            }),
        ),
        Device::new(
            "network1",
            DeviceKind::Network,
            "Loopback Alias",
            DeviceProperties::Network(NetworkInfo {
                network_type: Some("LoopBack".to_string()),
                mac: Some("11:22:33:44:55:66".to_string()),
                ..NetworkInfo::default()
            }),
        ),
        Device::new(
            "network2",
            DeviceKind::Network,
            "Zero MAC",
            DeviceProperties::Network(NetworkInfo {
                mac: Some("00:00:00:00:00:00".to_string()),
                ..NetworkInfo::default()
            }),
        ),
    ];

    assert!(component_keys_from_devices(&devices).is_empty());
}

#[test]
fn storage_falls_back_to_controller_model_when_device_model_is_missing() {
    let mut device = Device::new(
        "storage0",
        DeviceKind::Storage,
        "Boot Disk",
        DeviceProperties::Storage(StorageInfo {
            controller_model: Some("Samsung SSD 990 PRO".to_string()),
            ..StorageInfo::default()
        }),
    );
    device.serial = Some("DISK123".to_string());

    assert_eq!(
        component_keys_from_devices(&[device]),
        vec!["storage:model=Samsung SSD 990 PRO|serial=DISK123".to_string()]
    );
}

#[test]
fn gpu_falls_back_to_description_when_device_model_is_missing() {
    let device = Device::new(
        "gpu0",
        DeviceKind::Gpu,
        "Integrated Graphics",
        DeviceProperties::Gpu(GpuInfo {
            description: Some("RDNA 3 iGPU".to_string()),
            ..GpuInfo::default()
        }),
    );

    assert_eq!(
        component_keys_from_devices(&[device]),
        vec!["gpu:model=RDNA 3 iGPU|name=Integrated Graphics".to_string()]
    );
}
