use hw_model::{BusInfo, DeviceKind, DeviceProperties, DriverStatus, SourceKind, SourceStatus};
use hw_probe::{BiosProbe, GpuProbe, MemoryProbe, MonitorProbe, Probe, ProbeContext};
use hw_source::FakeSourceRunner;
use std::{path::PathBuf, time::Duration};

#[tokio::test]
async fn memory_probe_outputs_dimm_devices() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "memory"],
        "Memory Device\n\tSize: 16 GB\n\tLocator: ChannelA-DIMM0\n\tManufacturer: Samsung\n\tSerial Number: ABCD\n\tPart Number: M471A2K43\n\tType: DDR4\n\tSpeed: 3200 MT/s\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Memory);
}

#[tokio::test]
async fn memory_probe_uses_proc_meminfo_when_dmidecode_is_missing() {
    let runner =
        FakeSourceRunner::new().with_file("/proc/meminfo", "MemTotal:       16384000 kB\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Memory);
    match &result.devices[0].properties {
        DeviceProperties::Memory(memory) => {
            assert_eq!(memory.size_bytes, Some(16384000 * 1024));
        }
        other => panic!("expected memory properties, got {other:?}"),
    }
    assert!(result.devices[0].sources.iter().any(|source| {
        source.source == "/proc/meminfo"
            && source.kind == SourceKind::Procfs
            && source.status == SourceStatus::Success
    }));
    assert!(result.warnings.iter().any(|warning| {
        warning.code == "source_missing" && warning.source.as_deref() == Some("dmidecode -t memory")
    }));
    assert!(result.warnings.iter().any(|warning| {
        warning.code == "source_missing" && warning.source.as_deref() == Some("lshw -class memory")
    }));
}

#[tokio::test]
async fn memory_probe_uses_lshw_when_dmidecode_is_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "lshw",
        ["-class", "memory"],
        "*-memory\n\
             description: System Memory\n\
           *-bank:0\n\
                description: SODIMM DDR4 Synchronous 3200 MHz (0.3 ns)\n\
                product: M471A2K43CB1-CTD\n\
                vendor: Samsung\n\
                serial: ABCD1234\n\
                slot: ChannelA-DIMM0\n\
                size: 8GiB\n\
                clock: 3200MHz (0.3ns)\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Memory);
    match &result.devices[0].properties {
        DeviceProperties::Memory(memory) => {
            assert_eq!(memory.size_bytes, Some(8 * 1024 * 1024 * 1024));
            assert_eq!(memory.vendor.as_deref(), Some("Samsung"));
            assert_eq!(memory.memory_type.as_deref(), Some("DDR4"));
            assert_eq!(memory.speed_mtps, Some(3200));
            assert_eq!(memory.locator.as_deref(), Some("ChannelA-DIMM0"));
            assert_eq!(memory.serial.as_deref(), Some("ABCD1234"));
            assert_eq!(memory.part_number.as_deref(), Some("M471A2K43CB1-CTD"));
        }
        other => panic!("expected memory properties, got {other:?}"),
    }
    assert!(result.devices[0].sources.iter().any(|source| {
        source.source == "lshw -class memory"
            && source.kind == SourceKind::Command
            && source.status == SourceStatus::Success
    }));
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("dmidecode -t memory")
    );
}

#[tokio::test]
async fn bios_probe_outputs_bios_and_motherboard_devices() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "0,1,2,3"],
        "BIOS Information\n\tVendor: LENOVO\n\tVersion: N2IET98W\n\tRelease Date: 01/01/2026\nBase Board Information\n\tManufacturer: LENOVO\n\tProduct Name: 20XX\n\tSerial Number: BOARD123\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;
    assert!(result.devices.iter().any(|d| d.kind == DeviceKind::Bios));
    assert!(result
        .devices
        .iter()
        .any(|d| d.kind == DeviceKind::Motherboard));
}

#[tokio::test]
async fn bios_probe_does_not_emit_generic_devices_for_empty_dmi_output() {
    let runner = FakeSourceRunner::new().with_command("dmidecode", ["-t", "0,1,2,3"], "");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_empty");
}

#[tokio::test]
async fn bios_probe_uses_sysfs_dmi_when_dmidecode_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_file("/sys/class/dmi/id/bios_vendor", "LENOVO\n")
        .with_file("/sys/class/dmi/id/bios_version", "N2IET98W\n")
        .with_file("/sys/class/dmi/id/bios_date", "01/01/2026\n")
        .with_file("/sys/class/dmi/id/board_vendor", "LENOVO\n")
        .with_file("/sys/class/dmi/id/board_name", "20XX\n")
        .with_file("/sys/class/dmi/id/board_serial", "BOARD123\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 2);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("dmidecode -t 0,1,2,3")
    );

    let bios = result
        .devices
        .iter()
        .find(|device| device.kind == DeviceKind::Bios)
        .expect("expected bios device");
    assert_eq!(bios.name, "N2IET98W");
    assert!(bios.sources.iter().any(|source| {
        source.kind == SourceKind::Sysfs && source.source == "/sys/class/dmi/id"
    }));
    match &bios.properties {
        DeviceProperties::Bios(info) => {
            assert_eq!(info.vendor.as_deref(), Some("LENOVO"));
            assert_eq!(info.version.as_deref(), Some("N2IET98W"));
            assert_eq!(info.release_date.as_deref(), Some("01/01/2026"));
        }
        other => panic!("expected bios properties, got {other:?}"),
    }

    let board = result
        .devices
        .iter()
        .find(|device| device.kind == DeviceKind::Motherboard)
        .expect("expected motherboard device");
    assert_eq!(board.name, "20XX");
    assert!(board.sources.iter().any(|source| {
        source.kind == SourceKind::Sysfs && source.source == "/sys/class/dmi/id"
    }));
    match &board.properties {
        DeviceProperties::Motherboard(info) => {
            assert_eq!(info.manufacturer.as_deref(), Some("LENOVO"));
            assert_eq!(info.product_name.as_deref(), Some("20XX"));
            assert_eq!(info.serial.as_deref(), Some("BOARD123"));
        }
        other => panic!("expected motherboard properties, got {other:?}"),
    }
}

#[tokio::test]
async fn gpu_and_monitor_probes_output_devices() {
    let runner = FakeSourceRunner::new()
        .with_command("lspci", ["-nn", "-k"], "00:02.0 VGA compatible controller [0300]: Intel Corporation UHD Graphics [8086:9a49]\n\tKernel driver in use: i915\n")
        .with_command("xrandr", ["--query"], "eDP-1 connected primary 1920x1080+0+0\n   1920x1080     60.00*+\nHDMI-1 disconnected\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let gpu_result = GpuProbe.probe(&ctx).await;
    assert_eq!(gpu_result.devices[0].kind, DeviceKind::Gpu);
    match &gpu_result.devices[0].properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.vendor.as_deref(), Some("Intel"));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert_eq!(
        MonitorProbe.probe(&ctx).await.devices[0].kind,
        DeviceKind::Monitor
    );
}

#[tokio::test]
async fn gpu_probe_preserves_unknown_raw_device_description_as_vendor() {
    let runner = FakeSourceRunner::new().with_command(
        "lspci",
        ["-nn", "-k"],
        "00:03.0 Display controller [0380]: Acme Accelerant 9000 [1234:5678]\n\tKernel driver in use: acme\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let gpu_result = GpuProbe.probe(&ctx).await;

    assert_eq!(gpu_result.devices[0].kind, DeviceKind::Gpu);
    match &gpu_result.devices[0].properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.vendor.as_deref(), Some("Acme Accelerant 9000"));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn gpu_probe_normalizes_vendor_from_numeric_vendor_id_when_text_is_generic() {
    let runner = FakeSourceRunner::new().with_command(
        "lspci",
        ["-nn", "-k"],
        "00:03.0 Display controller [0380]: Device [1002:1638]\n\tKernel driver in use: amdgpu\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let gpu_result = GpuProbe.probe(&ctx).await;

    assert_eq!(gpu_result.devices[0].kind, DeviceKind::Gpu);
    match &gpu_result.devices[0].properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.vendor.as_deref(), Some("AMD"));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn gpu_probe_uses_sysfs_display_pci_when_lspci_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/bus/pci/devices/*",
            vec![PathBuf::from("/sys/bus/pci/devices/0000:00:02.0")],
        )
        .with_file("/sys/bus/pci/devices/0000:00:02.0/vendor", "0x8086\n")
        .with_file("/sys/bus/pci/devices/0000:00:02.0/device", "0x9a49\n")
        .with_file("/sys/bus/pci/devices/0000:00:02.0/class", "0x030000\n")
        .with_file(
            "/sys/bus/pci/devices/0000:00:02.0/subsystem_vendor",
            "0x1028\n",
        )
        .with_file(
            "/sys/bus/pci/devices/0000:00:02.0/subsystem_device",
            "0x087c\n",
        )
        .with_file("/sys/bus/pci/devices/0000:00:02.0/uevent", "DRIVER=i915\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "gpu:pci:0000:00:02.0");
    assert_eq!(result.devices[0].kind, DeviceKind::Gpu);
    assert_eq!(
        result.devices[0].bus,
        Some(BusInfo::Pci {
            address: "0000:00:02.0".to_string(),
            vendor_id: Some("8086".to_string()),
            device_id: Some("9a49".to_string()),
            subsystem_vendor_id: Some("1028".to_string()),
            subsystem_device_id: Some("087c".to_string()),
            class: Some("030000".to_string()),
        })
    );
    match &result.devices[0].properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.vendor.as_deref(), Some("Intel"));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("i915")
    );
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .map(|driver| driver.status),
        Some(DriverStatus::InUse)
    );
    assert_eq!(result.devices[0].sources[0].kind, SourceKind::Sysfs);
    assert_eq!(
        result.consumed[0].id, "pci:0000:00:02.0",
        "sysfs GPU fallback should consume its backing PCI device"
    );
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].source.as_deref(), Some("lspci -nn -k"));
}

#[tokio::test]
async fn gpu_probe_ignores_non_display_and_non_device_sysfs_pci_entries() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/bus/pci/devices/*",
            vec![
                PathBuf::from("/sys/bus/pci/devices/0000:00:1f.3"),
                PathBuf::from("/sys/bus/pci/devices/pci0000:00"),
            ],
        )
        .with_file("/sys/bus/pci/devices/0000:00:1f.3/class", "0x040300\n")
        .with_file("/sys/bus/pci/devices/pci0000:00/class", "0x030000\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    assert!(result.consumed.is_empty());
}

#[tokio::test]
async fn monitor_probe_uses_sysfs_edid_when_xrandr_verbose_is_missing() {
    let mut edid = vec![0u8; 128];
    edid[0..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
    edid[8] = 0x05;
    edid[9] = 0xe3;
    edid[16] = 12;
    edid[17] = 32;
    edid[21] = 52;
    edid[22] = 32;
    edid[72] = 0;
    edid[73] = 0;
    edid[74] = 0;
    edid[75] = 0xfc;
    edid[76] = 0;
    edid[77..90].copy_from_slice(b"AOC TEST    \n");
    let checksum = (256u16 - edid[..127].iter().map(|b| *b as u16).sum::<u16>() % 256) % 256;
    edid[127] = checksum as u8;

    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path, edid);
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("HDMI-1"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC"));
            assert_eq!(
                monitor.manufacturer_name.as_deref(),
                Some("AOC International")
            );
            assert_eq!(monitor.product.as_deref(), Some("AOC TEST"));
            assert_eq!(monitor.manufactured_year, Some(2022));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_uses_sysfs_edid_when_xrandr_query_is_missing() {
    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path.clone(), monitor_test_edid("AOC SYSFS"));
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "monitor:HDMI-1");
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(result.warnings[0].source.as_deref(), Some("xrandr --query"));
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("HDMI-1"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC"));
            assert_eq!(monitor.product.as_deref(), Some("AOC SYSFS"));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_does_not_create_sysfs_only_device_for_empty_edid() {
    let path = PathBuf::from("/sys/class/drm/card0-DP-1/edid");
    let runner = FakeSourceRunner::new()
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path.clone(), Vec::new());
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    let mut codes = result
        .warnings
        .iter()
        .map(|warning| warning.code.as_str())
        .collect::<Vec<_>>();
    codes.sort_unstable();
    assert_eq!(codes, vec!["edid_parse_failed", "source_missing"]);
}

#[tokio::test]
async fn monitor_probe_warns_when_sysfs_edid_read_fails() {
    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()]);
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some(path.to_str().unwrap())
    );
}

#[tokio::test]
async fn monitor_probe_skips_ambiguous_sysfs_edids_for_duplicate_normalized_connectors() {
    let first_path = PathBuf::from("/sys/class/drm/card0-DP-1/edid");
    let second_path = PathBuf::from("/sys/class/drm/card1-DP-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "DP-1 connected 2560x1440+0+0\n")
        .with_glob(
            "/sys/class/drm/*/edid",
            vec![first_path.clone(), second_path.clone()],
        )
        .with_file_bytes(first_path, monitor_test_edid("AOC FIRST"))
        .with_file_bytes(second_path, monitor_test_edid("AOC SECOND"));
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.warnings.is_empty());
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("DP-1"));
            assert_eq!(monitor.manufacturer, None);
            assert_eq!(monitor.product, None);
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_skips_ambiguous_sysfs_edids_even_when_only_one_duplicate_is_readable() {
    let readable_path = PathBuf::from("/sys/class/drm/card0-DP-1/edid");
    let unreadable_path = PathBuf::from("/sys/class/drm/card1-DP-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "DP-1 connected 2560x1440+0+0\n")
        .with_glob(
            "/sys/class/drm/*/edid",
            vec![readable_path.clone(), unreadable_path],
        )
        .with_file_bytes(readable_path, monitor_test_edid("AOC READABLE"));
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.warnings.is_empty());
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("DP-1"));
            assert_eq!(monitor.manufacturer, None);
            assert_eq!(monitor.product, None);
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_prefers_xrandr_verbose_edid_over_sysfs_edid() {
    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_command(
            "xrandr",
            ["--verbose"],
            format!(
                "HDMI-1 connected 1920x1080+0+0\n\tEDID:\n{}\n",
                xrandr_edid_hex(&monitor_test_edid("AOC VERBOSE"))
            ),
        )
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path, monitor_test_edid("AOC SYSFS"));
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("HDMI-1"));
            assert_eq!(monitor.product.as_deref(), Some("AOC VERBOSE"));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_falls_back_to_sysfs_when_xrandr_verbose_edid_is_invalid() {
    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_command(
            "xrandr",
            ["--verbose"],
            "HDMI-1 connected 1920x1080+0+0\n\tEDID:\n\t\t00ff\n",
        )
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path, monitor_test_edid("AOC SYSFS"));
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.warnings.len() <= 1);
    assert!(result
        .warnings
        .iter()
        .all(|warning| warning.code == "edid_parse_failed"));
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("HDMI-1"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC"));
            assert_eq!(
                monitor.manufacturer_name.as_deref(),
                Some("AOC International")
            );
            assert_eq!(monitor.product.as_deref(), Some("AOC SYSFS"));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_reports_edid_parse_warning_and_preserves_device() {
    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path.clone(), vec![0u8; 128]);
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "monitor:HDMI-1");
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "edid_parse_failed");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some(path.to_str().unwrap())
    );
    assert_eq!(
        result.warnings[0].device_id.as_deref(),
        Some("monitor:HDMI-1")
    );
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("HDMI-1"));
            assert_eq!(monitor.manufacturer, None);
            assert_eq!(monitor.product, None);
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

fn monitor_test_edid(name: &str) -> Vec<u8> {
    let mut edid = vec![0u8; 128];
    edid[0..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
    edid[8] = 0x05;
    edid[9] = 0xe3;
    edid[16] = 12;
    edid[17] = 32;
    edid[21] = 52;
    edid[22] = 32;
    edid[72] = 0;
    edid[73] = 0;
    edid[74] = 0;
    edid[75] = 0xfc;
    edid[76] = 0;
    let mut descriptor = [b' '; 13];
    let name = name.as_bytes();
    let len = name.len().min(12);
    descriptor[..len].copy_from_slice(&name[..len]);
    descriptor[12] = b'\n';
    edid[77..90].copy_from_slice(&descriptor);
    let checksum = (256u16 - edid[..127].iter().map(|b| *b as u16).sum::<u16>() % 256) % 256;
    edid[127] = checksum as u8;
    edid
}

fn xrandr_edid_hex(edid: &[u8]) -> String {
    edid.chunks(16)
        .map(|chunk| {
            let hex = chunk
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            format!("\t\t{hex}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
