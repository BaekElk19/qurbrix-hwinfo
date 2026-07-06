use async_trait::async_trait;
use hw_model::{Device, DeviceKind, DeviceProperties, DriverStatus, SourceKind, SourceStatus};
use hw_probe::{CpuProbe, NetworkProbe, Probe, ProbeContext, StorageProbe};
use hw_source::{
    CommandSpec, FakeSourceRunner, GlobResult, SourceBytesResult, SourceErrorKind, SourceResult,
    SourceRunner,
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

fn assert_source_status(device: &Device, source_name: &str, status: SourceStatus) {
    let source = device
        .sources
        .iter()
        .find(|source| source.source == source_name)
        .unwrap_or_else(|| panic!("expected source evidence for {source_name}"));
    assert_eq!(source.status, status);
}

fn warning_pairs(result: &hw_probe::ProbeResult) -> Vec<(Option<&str>, &str)> {
    let mut pairs = result
        .warnings
        .iter()
        .map(|warning| (warning.source.as_deref(), warning.code.as_str()))
        .collect::<Vec<_>>();
    pairs.sort_unstable();
    pairs
}

struct PermissionDeniedDmidecodeRunner {
    base: FakeSourceRunner,
}

#[async_trait]
impl SourceRunner for PermissionDeniedDmidecodeRunner {
    async fn run_command(&self, command: &CommandSpec, timeout: Duration) -> SourceResult {
        if command.program == "dmidecode" && command.args == ["-t", "4"] {
            SourceResult::error(
                command.display_name(),
                SourceErrorKind::PermissionDenied,
                "permission denied",
            )
        } else {
            self.base.run_command(command, timeout).await
        }
    }

    async fn read_file(&self, path: &Path) -> SourceResult {
        self.base.read_file(path).await
    }

    async fn read_file_bytes(&self, path: &Path) -> SourceBytesResult {
        self.base.read_file_bytes(path).await
    }

    async fn glob(&self, pattern: &str) -> GlobResult {
        self.base.glob(pattern).await
    }
}

#[tokio::test]
async fn cpu_probe_outputs_cpu_device() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lscpu",
            std::iter::empty::<&str>(),
            "Architecture: x86_64\nCPU(s): 8\nModel name: AMD Ryzen 7\nVendor ID: AuthenticAMD\nCore(s) per socket: 4\nSocket(s): 1\nCPU family: 25\nModel: 116\nStepping: 1\nBogoMIPS: 6587.42\nVirtualization: AMD-V\nFlags: fpu sse sse2\n",
        )
        .with_command(
            "lshw",
            ["-class", "processor"],
            "  *-cpu\n       description: CPU\n       product: AMD Ryzen 7 PRO\n       vendor: AMD\n",
        )
        .with_command(
            "dmidecode",
            ["-t", "4"],
            "Handle 0x0041, DMI type 4, 48 bytes\n\
             Processor Information\n\
             \tSocket Designation: CPU 0\n\
             \tManufacturer: Advanced Micro Devices, Inc.\n\
             \tVersion: Ryzen 7 PRO 7840U\n\
             \tCurrent Speed: 3300 MHz\n\
             \tCore Count: 8\n\
             \tThread Count: 16\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.warnings.is_empty());
    assert_eq!(result.devices[0].id, "cpu:0");
    assert_eq!(result.devices[0].kind, DeviceKind::Cpu);
    assert_eq!(result.devices[0].name, "Ryzen 7 PRO 7840U");
    assert_eq!(result.devices[0].sources.len(), 3);
    assert_source_status(&result.devices[0], "lscpu", SourceStatus::Success);
    assert_source_status(
        &result.devices[0],
        "lshw -class processor",
        SourceStatus::Success,
    );
    assert_source_status(&result.devices[0], "dmidecode -t 4", SourceStatus::Success);

    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.vendor.as_deref(), Some("AMD"));
            assert_eq!(cpu.architecture.as_deref(), Some("x86_64"));
            assert_eq!(cpu.cores, Some(8));
            assert_eq!(cpu.threads, Some(16));
            assert_eq!(cpu.sockets, Some(1));
            assert_eq!(cpu.current_freq_mhz, Some(3300));
            assert_eq!(cpu.family.as_deref(), Some("25"));
            assert_eq!(cpu.model.as_deref(), Some("116"));
            assert_eq!(cpu.stepping.as_deref(), Some("1"));
            assert_eq!(cpu.bogomips.as_deref(), Some("6587.42"));
            assert_eq!(cpu.virtualization.as_deref(), Some("AMD-V"));
            assert_eq!(cpu.flags, vec!["fpu", "sse", "sse2"]);
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_uses_dmi_when_lscpu_is_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "4"],
        "Handle 0x0041, DMI type 4, 48 bytes\n\
         Processor Information\n\
         \tSocket Designation: CPU 0\n\
         \tManufacturer: HiSilicon\n\
         \tVersion: Kunpeng 920\n\
         \tCurrent Speed: 2400 MHz\n\
         \tCore Count: 48\n\
         \tThread Count: 48\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].name, "Kunpeng 920");
    assert_eq!(result.warnings.len(), 2);
    assert_eq!(
        warning_pairs(&result),
        vec![
            (Some("lscpu"), "source_missing"),
            (Some("lshw -class processor"), "source_missing"),
        ]
    );
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_source_status(&result.devices[0], "dmidecode -t 4", SourceStatus::Success);
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.vendor.as_deref(), Some("HiSilicon"));
            assert_eq!(cpu.current_freq_mhz, Some(2400));
            assert_eq!(cpu.cores, Some(48));
            assert_eq!(cpu.threads, Some(48));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_reports_warnings_when_optional_sources_are_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "lscpu",
        std::iter::empty::<&str>(),
        "Architecture: amd64\nCPU(s): 8\nModel name: AMD Ryzen 7\nVendor ID: AuthenticAMD\nCore(s) per socket: 4\nSocket(s): 1\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_source_status(&result.devices[0], "lscpu", SourceStatus::Success);
    assert_eq!(result.warnings.len(), 2);
    assert_eq!(
        warning_pairs(&result),
        vec![
            (Some("dmidecode -t 4"), "source_missing"),
            (Some("lshw -class processor"), "source_missing"),
        ]
    );
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.vendor.as_deref(), Some("AMD"));
            assert_eq!(cpu.architecture.as_deref(), Some("x86_64"));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_ignores_family_only_dmi_for_evidence() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lscpu",
            std::iter::empty::<&str>(),
            "Architecture: x86_64\nCPU(s): 8\nModel name: AMD Ryzen 7\nVendor ID: AuthenticAMD\nCore(s) per socket: 4\nSocket(s): 1\n",
        )
        .with_command("lshw", ["-class", "processor"], "")
        .with_command(
            "dmidecode",
            ["-t", "4"],
            "Handle 0x0041, DMI type 4, 48 bytes\n\
             Processor Information\n\
             \tFamily: Server\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.warnings.is_empty());
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_source_status(&result.devices[0], "lscpu", SourceStatus::Success);
}

#[tokio::test]
async fn cpu_probe_infers_vendor_from_dmi_name_when_manufacturer_is_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "4"],
        "Handle 0x0041, DMI type 4, 48 bytes\n\
         Processor Information\n\
         \tSocket Designation: CPU 0\n\
         \tVersion: Kunpeng 920\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].name, "Kunpeng 920");
    assert_eq!(result.warnings.len(), 2);
    assert_eq!(
        warning_pairs(&result),
        vec![
            (Some("lscpu"), "source_missing"),
            (Some("lshw -class processor"), "source_missing"),
        ]
    );
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_source_status(&result.devices[0], "dmidecode -t 4", SourceStatus::Success);
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.vendor.as_deref(), Some("HiSilicon"));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_warns_when_dmidecode_is_permission_denied() {
    let runner = PermissionDeniedDmidecodeRunner {
        base: FakeSourceRunner::new().with_command(
            "lscpu",
            std::iter::empty::<&str>(),
            "Architecture: x86_64\nCPU(s): 8\nModel name: AMD Ryzen 7\nVendor ID: AuthenticAMD\nCore(s) per socket: 4\nSocket(s): 1\n",
        ),
    };
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.warnings.len(), 2);
    assert_eq!(
        warning_pairs(&result),
        vec![
            (Some("dmidecode -t 4"), "source_permission_denied"),
            (Some("lshw -class processor"), "source_missing"),
        ]
    );
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_source_status(&result.devices[0], "lscpu", SourceStatus::Success);
}

#[tokio::test]
async fn cpu_probe_uses_proc_cpuinfo_when_command_sources_are_missing() {
    let runner = FakeSourceRunner::new().with_file(
        "/proc/cpuinfo",
        "processor\t: 0\n\
         BogoMIPS\t: 100.00\n\
         Features\t: fp asimd evtstrm crc32\n\
         CPU implementer\t: 0x70\n\
         CPU architecture: 8\n\
         CPU part\t: 0x660\n\
         CPU revision\t: 2\n\
         cpu MHz\t\t: 2300.000\n\
         \n\
         processor\t: 1\n\
         BogoMIPS\t: 100.00\n\
         Features\t: fp asimd evtstrm crc32\n\
         CPU implementer\t: 0x70\n\
         CPU architecture: 8\n\
         CPU part\t: 0x660\n\
         CPU revision\t: 2\n\
         cpu MHz\t\t: 2300.000\n\
         \n\
         Hardware\t: Phytium D2000/8\n\
         Processor\t: AArch64 Processor rev 2 (aarch64)\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].name, "Phytium D2000/8");
    assert_eq!(result.devices[0].sources.len(), 1);
    let source = result.devices[0]
        .sources
        .iter()
        .find(|source| source.source == "/proc/cpuinfo")
        .expect("expected /proc/cpuinfo source evidence");
    assert_eq!(source.kind, SourceKind::Procfs);
    assert_eq!(source.status, SourceStatus::Success);
    assert_eq!(
        warning_pairs(&result),
        vec![
            (Some("dmidecode -t 4"), "source_missing"),
            (Some("lscpu"), "source_missing"),
            (Some("lshw -class processor"), "source_missing"),
        ]
    );
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.name.as_deref(), Some("Phytium D2000/8"));
            assert_eq!(cpu.vendor.as_deref(), Some("Phytium"));
            assert_eq!(cpu.architecture.as_deref(), Some("aarch64"));
            assert_eq!(cpu.threads, Some(2));
            assert_eq!(cpu.current_freq_mhz, Some(2300));
            assert_eq!(cpu.flags, vec!["fp", "asimd", "evtstrm", "crc32"]);
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_uses_proc_hardware_kirin_when_other_sources_are_missing() {
    let runner =
        FakeSourceRunner::new().with_file("/proc/hardware", "Hardware\t: HUAWEI Kirin 9006C\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].name, "HUAWEI Kirin 9006C");
    assert_eq!(result.devices[0].sources.len(), 1);
    let source = result.devices[0]
        .sources
        .iter()
        .find(|source| source.source == "/proc/hardware")
        .expect("expected /proc/hardware source evidence");
    assert_eq!(source.kind, SourceKind::Procfs);
    assert_eq!(source.status, SourceStatus::Success);
    assert_eq!(
        warning_pairs(&result),
        vec![
            (Some("dmidecode -t 4"), "source_missing"),
            (Some("lscpu"), "source_missing"),
            (Some("lshw -class processor"), "source_missing"),
        ]
    );
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.name.as_deref(), Some("HUAWEI Kirin 9006C"));
            assert_eq!(cpu.vendor.as_deref(), Some("HiSilicon"));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn network_probe_outputs_network_device() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "ip",
            ["-j", "link"],
            r#"[{"ifname":"wlan0","address":"aa:bb:cc:dd:ee:ff","operstate":"UP","mtu":1500}]"#,
        )
        .with_file("/sys/class/net/wlan0/speed", "867\n")
        .with_file("/sys/class/net/wlan0/duplex", "full\n")
        .with_file("/sys/class/net/wlan0/device/uevent", "DRIVER=iwlwifi\n")
        .with_glob(
            "/sys/class/net/wlan0/wireless",
            vec![PathBuf::from("/sys/class/net/wlan0/wireless")],
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = NetworkProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Network);
    assert_eq!(result.devices[0].capabilities, vec!["wireless"]);
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("iwlwifi")
    );
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .map(|driver| driver.status),
        Some(DriverStatus::InUse)
    );
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Sysfs && source.source == "/sys/class/net/wlan0"));
    assert_eq!(
        result.devices[0]
            .sources
            .iter()
            .filter(|source| source.source == "/sys/class/net/wlan0")
            .count(),
        1
    );
    match &result.devices[0].properties {
        DeviceProperties::Network(network) => {
            assert_eq!(network.interface.as_deref(), Some("wlan0"));
            assert_eq!(network.network_type.as_deref(), Some("wireless"));
            assert_eq!(network.speed_mbps, Some(867));
            assert_eq!(network.duplex.as_deref(), Some("full"));
        }
        other => panic!("expected network properties, got {other:?}"),
    }
}

#[tokio::test]
async fn network_probe_reads_ip_addresses_from_ip_addr() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "ip",
            ["-j", "link"],
            r#"[{"ifname":"wlan0","address":"aa:bb:cc:dd:ee:ff","operstate":"UP","mtu":1500}]"#,
        )
        .with_command(
            "ip",
            ["-j", "addr"],
            r#"[
                {
                    "ifname":"wlan0",
                    "addr_info":[
                        {"family":"inet","local":"192.168.1.23","prefixlen":24},
                        {"family":"inet6","local":"fe80::1234","prefixlen":64}
                    ]
                }
            ]"#,
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = NetworkProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.warnings.is_empty());
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "ip -j addr"));
    match &result.devices[0].properties {
        DeviceProperties::Network(network) => {
            assert_eq!(network.ipv4, vec!["192.168.1.23"]);
            assert_eq!(network.ipv6, vec!["fe80::1234"]);
        }
        other => panic!("expected network properties, got {other:?}"),
    }
}

#[tokio::test]
async fn network_probe_marks_ethernet_capability_from_sysfs() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "ip",
            ["-j", "link"],
            r#"[{"ifname":"eth0","address":"aa:bb:cc:dd:ee:ff","operstate":"UP","mtu":1500}]"#,
        )
        .with_command("ip", ["-j", "addr"], "[]")
        .with_file("/sys/class/net/eth0/speed", "1000\n")
        .with_file("/sys/class/net/eth0/duplex", "full\n")
        .with_file("/sys/class/net/eth0/device/uevent", "DRIVER=e1000e\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = NetworkProbe.probe(&ctx).await;

    assert_eq!(result.devices[0].capabilities, vec!["ethernet"]);
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Sysfs && source.source == "/sys/class/net/eth0"));
    match &result.devices[0].properties {
        DeviceProperties::Network(network) => {
            assert_eq!(network.network_type.as_deref(), Some("ethernet"));
        }
        other => panic!("expected network properties, got {other:?}"),
    }
}

#[tokio::test]
async fn network_probe_filters_loopback_and_common_virtual_interfaces() {
    let runner = FakeSourceRunner::new().with_command(
        "ip",
        ["-j", "link"],
        r#"[
            {"ifname":"lo","address":"00:00:00:00:00:00","operstate":"UNKNOWN","mtu":65536},
            {"ifname":"docker0","address":"02:42:aa:bb:cc:dd","operstate":"DOWN","mtu":1500},
            {"ifname":"veth1234","address":"aa:bb:cc:dd:ee:01","operstate":"UP","mtu":1500},
            {"ifname":"eth0","address":"aa:bb:cc:dd:ee:ff","operstate":"UP","mtu":1500}
        ]"#,
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = NetworkProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].name, "eth0");
}

#[tokio::test]
async fn network_probe_uses_sysfs_when_ip_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/class/net/*",
            vec![
                PathBuf::from("/sys/class/net/enp1s0"),
                PathBuf::from("/sys/class/net/lo"),
                PathBuf::from("/sys/class/net/veth1234"),
            ],
        )
        .with_file("/sys/class/net/enp1s0/address", "aa:bb:cc:dd:ee:ff\n")
        .with_file("/sys/class/net/enp1s0/operstate", "up\n")
        .with_file("/sys/class/net/enp1s0/speed", "1000\n")
        .with_file("/sys/class/net/enp1s0/duplex", "full\n")
        .with_file("/sys/class/net/enp1s0/device/uevent", "DRIVER=e1000e\n")
        .with_glob(
            "/sys/class/net/enp1s0/wireless",
            vec![PathBuf::from("/sys/class/net/enp1s0/wireless")],
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = NetworkProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(
        warning_pairs(&result),
        vec![(Some("ip -j link"), "source_missing")]
    );

    let device = &result.devices[0];
    assert_eq!(device.kind, DeviceKind::Network);
    assert_eq!(device.name, "enp1s0");
    assert_eq!(device.sources.len(), 1);
    let source = device
        .sources
        .iter()
        .find(|source| source.source == "/sys/class/net/enp1s0")
        .expect("expected sysfs source evidence");
    assert_eq!(source.kind, SourceKind::Sysfs);
    assert_eq!(source.status, SourceStatus::Success);
    assert_eq!(
        device
            .sources
            .iter()
            .filter(|source| source.source == "/sys/class/net/enp1s0")
            .count(),
        1
    );
    assert_eq!(device.capabilities, vec!["wireless"]);
    assert_eq!(
        device
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("e1000e")
    );
    assert_eq!(
        device.driver.as_ref().map(|driver| driver.status),
        Some(DriverStatus::InUse)
    );

    match &device.properties {
        DeviceProperties::Network(network) => {
            assert_eq!(network.interface.as_deref(), Some("enp1s0"));
            assert_eq!(network.mac.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
            assert_eq!(network.operstate.as_deref(), Some("up"));
            assert_eq!(network.speed_mbps, Some(1000));
            assert_eq!(network.duplex.as_deref(), Some("full"));
            assert!(network.ipv4.is_empty());
            assert!(network.ipv6.is_empty());
        }
        other => panic!("expected network properties, got {other:?}"),
    }
}

#[tokio::test]
async fn network_probe_warns_when_json_output_is_malformed() {
    let runner = FakeSourceRunner::new().with_command("ip", ["-j", "link"], "not json");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = NetworkProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "parse_failed");
    assert_eq!(result.warnings[0].source.as_deref(), Some("ip -j link"));
}

#[tokio::test]
async fn storage_probe_outputs_storage_device() {
    let runner = FakeSourceRunner::new().with_command(
        "lsblk",
        ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
        r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"Disk","serial":"S1","tran":"sata"}]}"#,
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Storage);
}

#[tokio::test]
async fn storage_probe_preserves_wwn_and_firmware_from_lsblk_success_path() {
    let runner = FakeSourceRunner::new().with_command(
        "lsblk",
        ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
        r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"Disk","serial":"S1","tran":"sata","wwn":"0x5002538F00000000","rev":"1.0A"}]}"#,
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "storage:wwn:5002538F00000000");
    assert_eq!(result.devices[0].serial.as_deref(), Some("S1"));
    assert_eq!(result.devices[0].model.as_deref(), Some("Disk"));
    let DeviceProperties::Storage(storage) = &result.devices[0].properties else {
        panic!("expected storage properties");
    };
    assert_eq!(storage.wwn.as_deref(), Some("5002538F00000000"));
    assert_eq!(storage.firmware.as_deref(), Some("1.0A"));
}

#[tokio::test]
async fn storage_probe_reads_smart_health_and_temperature() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"Disk","serial":"S1","tran":"sata"}]}"#,
        )
        .with_command(
            "smartctl",
            ["-a", "-j", "/dev/sda"],
            r#"{"smart_status":{"passed":true},"temperature":{"current":37}}"#,
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let DeviceProperties::Storage(storage) = &result.devices[0].properties else {
        panic!("expected storage properties");
    };
    assert_eq!(storage.smart_status.as_deref(), Some("passed"));
    assert_eq!(storage.temperature_celsius, Some(37.0));
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command
            && source.source == "smartctl -a -j /dev/sda"));
}

#[tokio::test]
async fn storage_probe_reads_smart_health_from_nonzero_smartctl_json() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"Disk","serial":"S1","tran":"sata"}]}"#,
        )
        .with_command_status(
            "smartctl",
            ["-a", "-j", "/dev/sda"],
            r#"{"smart_status":{"passed":false},"temperature":{"current":44}}"#,
            8,
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let DeviceProperties::Storage(storage) = &result.devices[0].properties else {
        panic!("expected storage properties");
    };
    assert_eq!(storage.smart_status.as_deref(), Some("failed"));
    assert_eq!(storage.temperature_celsius, Some(44.0));
    let smart_source = result.devices[0]
        .sources
        .iter()
        .find(|source| source.source == "smartctl -a -j /dev/sda")
        .expect("expected smartctl source evidence");
    assert_eq!(smart_source.status, SourceStatus::Failed);
}

#[tokio::test]
async fn storage_probe_reads_driver_from_sysfs_for_lsblk_disk() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"Disk","serial":"S1","tran":"sata"}]}"#,
        )
        .with_file("/sys/block/sda/device/uevent", "DRIVER=sd\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("sd")
    );
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .map(|driver| driver.status),
        Some(DriverStatus::InUse)
    );
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Sysfs && source.source == "/sys/block/sda"));
}

#[tokio::test]
async fn storage_probe_uses_sysfs_when_lsblk_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/block/*",
            vec![
                PathBuf::from("/sys/block/sda"),
                PathBuf::from("/sys/block/loop0"),
                PathBuf::from("/sys/block/ram0"),
                PathBuf::from("/sys/block/sr0"),
                PathBuf::from("/sys/block/zram0"),
                PathBuf::from("/sys/block/dm-0"),
                PathBuf::from("/sys/block/md0"),
            ],
        )
        .with_file("/sys/block/sda/size", "2097152\n")
        .with_file("/sys/block/sda/device/vendor", "Samsung\n")
        .with_file("/sys/block/sda/device/model", "Samsung SSD 980\n")
        .with_file("/sys/block/sda/device/serial", "S12345\n")
        .with_file("/sys/block/sda/device/wwid", "naa.5002538f00000000\n")
        .with_file("/sys/block/sda/device/rev", "3B2QGXA7\n")
        .with_file("/sys/block/sda/device/uevent", "DRIVER=sd\n")
        .with_file("/sys/block/sda/queue/rotational", "0\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "storage:serial:S12345");
    assert_eq!(result.devices[0].kind, DeviceKind::Storage);
    assert_eq!(result.devices[0].name, "Samsung SSD 980");
    assert_eq!(result.devices[0].vendor.as_deref(), Some("Samsung"));
    assert_eq!(result.devices[0].model.as_deref(), Some("Samsung SSD 980"));
    assert_eq!(result.devices[0].serial.as_deref(), Some("S12345"));
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("sd")
    );
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .map(|driver| driver.status),
        Some(DriverStatus::InUse)
    );
    assert_eq!(
        warning_pairs(&result),
        vec![(
            Some("lsblk -J -b -o NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"),
            "source_missing"
        )]
    );

    match &result.devices[0].properties {
        DeviceProperties::Storage(storage) => {
            assert_eq!(storage.device_node.as_deref(), Some("/dev/sda"));
            assert_eq!(storage.size_bytes, Some(1_073_741_824));
            assert_eq!(storage.media_type.as_deref(), Some("ssd"));
            assert_eq!(storage.wwn.as_deref(), Some("naa.5002538f00000000"));
            assert_eq!(storage.firmware.as_deref(), Some("3B2QGXA7"));
        }
        other => panic!("expected storage properties, got {other:?}"),
    }

    let source = result.devices[0]
        .sources
        .iter()
        .find(|source| source.source == "/sys/block/sda")
        .expect("expected /sys/block/sda source evidence");
    assert_eq!(source.kind, SourceKind::Sysfs);
    assert_eq!(source.status, SourceStatus::Success);
    assert_eq!(
        result.devices[0]
            .sources
            .iter()
            .filter(|source| source.source == "/sys/block/sda")
            .count(),
        1
    );
}

#[tokio::test]
async fn storage_probe_uses_block_wwid_and_firmware_rev_sysfs_fallbacks() {
    let runner = FakeSourceRunner::new()
        .with_glob("/sys/block/*", vec![PathBuf::from("/sys/block/nvme0n1")])
        .with_file("/sys/block/nvme0n1/device/model", "NVMe Disk\n")
        .with_file("/sys/block/nvme0n1/wwid", "0X5002538F00000000\n")
        .with_file("/sys/block/nvme0n1/device/firmware_rev", "1.0A\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "storage:dev:/dev/nvme0n1");
    let DeviceProperties::Storage(storage) = &result.devices[0].properties else {
        panic!("expected storage properties");
    };
    assert_eq!(storage.wwn.as_deref(), Some("5002538F00000000"));
    assert_eq!(storage.firmware.as_deref(), Some("1.0A"));
}

#[tokio::test]
async fn storage_probe_warns_when_json_output_is_malformed() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            "not json",
        )
        .with_glob("/sys/block/*", vec![PathBuf::from("/sys/block/sda")])
        .with_file("/sys/block/sda/size", "2097152\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "parse_failed");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("lsblk -J -b -o NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV")
    );
}
