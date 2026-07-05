use hw_model::{DeviceKind, DeviceProperties};
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
