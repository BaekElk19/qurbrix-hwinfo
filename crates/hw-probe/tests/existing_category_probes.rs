use async_trait::async_trait;
use hw_model::{
    BusInfo, Device, DeviceKind, DeviceProperties, DriverStatus, SourceKind, SourceStatus,
};
use hw_probe::{CpuProbe, NetworkProbe, Probe, ProbeContext, StorageProbe, SystemProbe};
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

#[tokio::test]
async fn system_probe_outputs_runtime_and_dmi_system_fields() {
    let runner = FakeSourceRunner::new()
        .with_file("/proc/sys/kernel/hostname", "deepin-host\n")
        .with_file("/etc/os-release", "PRETTY_NAME=\"Deepin 25\"\n")
        .with_command("uname", ["-r"], "6.12.1-amd64\n")
        .with_command("uname", ["-m"], "x86_64\n")
        .with_command(
            "dmidecode",
            ["-t", "1"],
            "System Information\n\
                 \tManufacturer: LENOVO\n\
                 \tProduct Name: ThinkPad X1\n\
                 \tVersion: ThinkPad X1 Carbon Gen 12\n\
                 \tSerial Number: SYS123\n\
                 \tUUID: 11111111-2222-3333-4444-555555555555\n\
                 \tWake-up Type: Power Switch\n\
                 \tSKU Number: SKU123\n\
                 \tFamily: ThinkPad X1\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = SystemProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    let device = &result.devices[0];
    assert_eq!(device.id, "system:serial:SYS123");
    assert_eq!(device.kind, DeviceKind::System);
    assert_eq!(device.name, "ThinkPad X1");
    assert_eq!(device.vendor.as_deref(), Some("LENOVO"));
    assert_eq!(device.model.as_deref(), Some("ThinkPad X1"));
    assert_eq!(device.serial.as_deref(), Some("SYS123"));
    match &device.properties {
        DeviceProperties::System(info) => {
            assert_eq!(info.hostname.as_deref(), Some("deepin-host"));
            assert_eq!(info.os.as_deref(), Some("Deepin 25"));
            assert_eq!(info.kernel.as_deref(), Some("6.12.1-amd64"));
            assert_eq!(info.architecture.as_deref(), Some("x86_64"));
            assert_eq!(info.manufacturer.as_deref(), Some("LENOVO"));
            assert_eq!(info.product_name.as_deref(), Some("ThinkPad X1"));
            assert_eq!(info.version.as_deref(), Some("ThinkPad X1 Carbon Gen 12"));
            assert_eq!(info.serial.as_deref(), Some("SYS123"));
            assert_eq!(
                info.uuid.as_deref(),
                Some("11111111-2222-3333-4444-555555555555")
            );
            assert_eq!(info.wake_up_type.as_deref(), Some("Power Switch"));
            assert_eq!(info.sku_number.as_deref(), Some("SKU123"));
            assert_eq!(info.family.as_deref(), Some("ThinkPad X1"));
        }
        other => panic!("expected system properties, got {other:?}"),
    }
    assert!(device
        .sources
        .iter()
        .any(|source| { source.kind == SourceKind::Command && source.source == "dmidecode -t 1" }));
}

#[tokio::test]
async fn system_probe_uses_sysfs_dmi_when_dmidecode_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_file("/sys/class/dmi/id/sys_vendor", "LENOVO\n")
        .with_file("/sys/class/dmi/id/product_name", "ThinkPad X1\n")
        .with_file(
            "/sys/class/dmi/id/product_version",
            "ThinkPad X1 Carbon Gen 12\n",
        )
        .with_file("/sys/class/dmi/id/product_serial", "SYS123\n")
        .with_file(
            "/sys/class/dmi/id/product_uuid",
            "11111111-2222-3333-4444-555555555555\n",
        )
        .with_file("/sys/class/dmi/id/product_sku", "SKU123\n")
        .with_file("/sys/class/dmi/id/product_family", "ThinkPad X1\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = SystemProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "system:serial:SYS123");
    assert!(result.devices[0].sources.iter().any(|source| {
        source.kind == SourceKind::Sysfs && source.source == "/sys/class/dmi/id"
    }));
    assert!(result.warnings.iter().any(|warning| {
        warning.code == "source_missing" && warning.source.as_deref() == Some("dmidecode -t 1")
    }));
    match &result.devices[0].properties {
        DeviceProperties::System(info) => {
            assert_eq!(info.manufacturer.as_deref(), Some("LENOVO"));
            assert_eq!(info.product_name.as_deref(), Some("ThinkPad X1"));
            assert_eq!(info.version.as_deref(), Some("ThinkPad X1 Carbon Gen 12"));
            assert_eq!(info.serial.as_deref(), Some("SYS123"));
            assert_eq!(
                info.uuid.as_deref(),
                Some("11111111-2222-3333-4444-555555555555")
            );
            assert_eq!(info.sku_number.as_deref(), Some("SKU123"));
            assert_eq!(info.family.as_deref(), Some("ThinkPad X1"));
        }
        other => panic!("expected system properties, got {other:?}"),
    }
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

    async fn canonicalize_path(&self, path: &Path) -> SourceResult {
        self.base.canonicalize_path(path).await
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
            "Architecture: x86_64\nCPU(s): 8\nOn-line CPU(s) list: 0-3,6\nModel name: AMD Ryzen 7\nVendor ID: AuthenticAMD\nCore(s) per socket: 4\nThread(s) per core: 2\nSocket(s): 1\nCPU family: 25\nModel: 116\nStepping: 1\nBogoMIPS: 6587.42\nVirtualization: AMD-V\nL1d cache: 256 KiB (8 instances)\nL1i cache: 256 KiB (8 instances)\nL2 cache: 8 MiB (8 instances)\nL3 cache: 16 MiB (1 instance)\nFlags: fpu sse sse2\n",
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
             \tCore Enabled: 8\n\
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
            assert_eq!(cpu.threads_per_core, Some(2));
            assert_eq!(cpu.sockets, Some(1));
            assert_eq!(cpu.enabled_cores, Some(8));
            assert_eq!(cpu.online_threads, Some(5));
            assert_eq!(cpu.online_cores, Some(3));
            assert_eq!(cpu.current_freq_mhz, Some(3300));
            assert_eq!(cpu.family.as_deref(), Some("25"));
            assert_eq!(cpu.model.as_deref(), Some("116"));
            assert_eq!(cpu.stepping.as_deref(), Some("1"));
            assert_eq!(cpu.bogomips.as_deref(), Some("6587.42"));
            assert_eq!(cpu.virtualization.as_deref(), Some("AMD-V"));
            assert_eq!(cpu.l1d_cache.as_deref(), Some("256 KiB (8 instances)"));
            assert_eq!(cpu.l1i_cache.as_deref(), Some("256 KiB (8 instances)"));
            assert_eq!(cpu.l2_cache.as_deref(), Some("8 MiB (8 instances)"));
            assert_eq!(cpu.l3_cache.as_deref(), Some("16 MiB (1 instance)"));
            assert_eq!(cpu.flags, vec!["fpu", "sse", "sse2"]);
            assert_eq!(cpu.extensions, vec!["SSE", "SSE2"]);
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
         \tFamily: ARMv8\n\
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
            assert_eq!(cpu.family.as_deref(), Some("ARMv8"));
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
    assert_eq!(
        warning_pairs(&result),
        vec![
            (Some("dmidecode -t 4"), "source_empty"),
            (Some("lshw -class processor"), "source_empty"),
        ]
    );
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_source_status(&result.devices[0], "lscpu", SourceStatus::Success);
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.family.as_deref(), Some("Server"));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
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
async fn cpu_probe_preserves_kylin_dmi_slot_serial_and_external_clock() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "4"],
        "Handle 0x0041, DMI type 4, 48 bytes\n\
         Processor Information\n\
         \tSocket Designation: CPU 0\n\
         \tManufacturer: GenuineIntel\n\
         \tVersion: Intel(R) Core(TM) i7-1185G7\n\
         \tSerial Number: CPU-SERIAL-1\n\
         \tExternal Clock: 100 MHz\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.socket_designations, vec!["CPU 0"]);
            assert_eq!(cpu.serial_numbers, vec!["CPU-SERIAL-1"]);
            assert_eq!(cpu.external_clock_mhz, Some(100));
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
         clflush size\t: 64\n\
         cache size\t: 2048 KB\n\
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
    assert_eq!(result.devices[0].name, "Phytium D2000");
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
            assert_eq!(cpu.name.as_deref(), Some("Phytium D2000"));
            assert_eq!(cpu.vendor.as_deref(), Some("Phytium"));
            assert_eq!(cpu.architecture.as_deref(), Some("aarch64"));
            assert_eq!(cpu.threads, Some(2));
            assert_eq!(cpu.current_freq_mhz, Some(2300));
            assert_eq!(cpu.clflush_size_bytes, Some(64));
            assert_eq!(cpu.l2_cache.as_deref(), Some("2048 KB"));
            assert_eq!(cpu.flags, vec!["fp", "asimd", "evtstrm", "crc32"]);
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_uses_proc_cpuinfo_when_lscpu_parses_empty() {
    let runner = FakeSourceRunner::new()
        .with_command("lscpu", std::iter::empty::<&str>(), "")
        .with_file(
            "/proc/cpuinfo",
            "processor\t: 0\n\
             model name\t: Intel(R) Core(TM) i7-1185G7 @ 3.00GHz\n\
             vendor_id\t: GenuineIntel\n\
             cpu MHz\t\t: 3000.000\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(
        warning_pairs(&result),
        vec![
            (Some("dmidecode -t 4"), "source_missing"),
            (Some("lscpu"), "source_empty"),
            (Some("lshw -class processor"), "source_missing"),
        ]
    );
    assert_eq!(result.devices[0].sources.len(), 1);
    assert_source_status(&result.devices[0], "/proc/cpuinfo", SourceStatus::Success);
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(
                cpu.name.as_deref(),
                Some("Intel(R) Core(TM) i7-1185G7 @ 3.00GHz")
            );
            assert_eq!(cpu.vendor.as_deref(), Some("Intel"));
            assert_eq!(cpu.current_freq_mhz, Some(3000));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_exposes_deepin_arm_cpu_identity_fields_from_proc_cpuinfo() {
    let runner = FakeSourceRunner::new().with_file(
        "/proc/cpuinfo",
        "processor\t: 0\n\
         BogoMIPS\t: 100.00\n\
         Features\t: fp asimd evtstrm crc32\n\
         CPU implementer\t: 0x70\n\
         CPU architecture: 8\n\
         CPU variant\t: 0x1\n\
         CPU part\t: 0x660\n\
         CPU revision\t: 2\n\
         \n\
         Hardware\t: Phytium D2000/8\n\
         Processor\t: AArch64 Processor rev 2 (aarch64)\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.cpu_implementer.as_deref(), Some("0x70"));
            assert_eq!(cpu.cpu_architecture.as_deref(), Some("8"));
            assert_eq!(cpu.cpu_variant.as_deref(), Some("0x1"));
            assert_eq!(cpu.cpu_part.as_deref(), Some("0x660"));
            assert_eq!(cpu.cpu_revision.as_deref(), Some("2"));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_uses_proc_cpuinfo_topology_when_lscpu_is_missing() {
    let runner = FakeSourceRunner::new().with_file(
        "/proc/cpuinfo",
        "processor\t: 0\n\
         model name\t: Intel(R) Core(TM) i7-1185G7 @ 3.00GHz\n\
         vendor_id\t: GenuineIntel\n\
         physical id\t: 0\n\
         siblings\t: 4\n\
         cpu cores\t: 2\n\
         \n\
         processor\t: 1\n\
         model name\t: Intel(R) Core(TM) i7-1185G7 @ 3.00GHz\n\
         vendor_id\t: GenuineIntel\n\
         physical id\t: 0\n\
         siblings\t: 4\n\
         cpu cores\t: 2\n\
         \n\
         processor\t: 2\n\
         model name\t: Intel(R) Core(TM) i7-1185G7 @ 3.00GHz\n\
         vendor_id\t: GenuineIntel\n\
         physical id\t: 0\n\
         siblings\t: 4\n\
         cpu cores\t: 2\n\
         \n\
         processor\t: 3\n\
         model name\t: Intel(R) Core(TM) i7-1185G7 @ 3.00GHz\n\
         vendor_id\t: GenuineIntel\n\
         physical id\t: 0\n\
         siblings\t: 4\n\
         cpu cores\t: 2\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_source_status(&result.devices[0], "/proc/cpuinfo", SourceStatus::Success);
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.vendor.as_deref(), Some("Intel"));
            assert_eq!(cpu.cores, Some(2));
            assert_eq!(cpu.threads, Some(4));
            assert_eq!(cpu.threads_per_core, Some(2));
            assert_eq!(cpu.sockets, Some(1));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_keeps_lscpu_identity_when_proc_cpuinfo_disagrees() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lscpu",
            std::iter::empty::<&str>(),
            "Architecture: x86_64\n\
             CPU(s): 8\n\
             Model name: Intel(R) Core(TM) i7-1185G7\n\
             Vendor ID: GenuineIntel\n\
             Core(s) per socket: 4\n\
             Socket(s): 1\n",
        )
        .with_file(
            "/proc/cpuinfo",
            "processor\t: 0\n\
             model name\t: Bogus Proc CPU\n\
             vendor_id\t: AuthenticAMD\n\
             cpu MHz\t\t: 3000.000\n\
             flags\t\t: fpu sse sse2\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_source_status(&result.devices[0], "lscpu", SourceStatus::Success);
    assert_source_status(&result.devices[0], "/proc/cpuinfo", SourceStatus::Success);
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.name.as_deref(), Some("Intel(R) Core(TM) i7-1185G7"));
            assert_eq!(cpu.vendor.as_deref(), Some("Intel"));
            assert_eq!(cpu.current_freq_mhz, Some(3000));
            assert_eq!(cpu.flags, vec!["fpu", "sse", "sse2"]);
            assert_eq!(cpu.extensions, vec!["SSE", "SSE2"]);
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_warns_when_proc_cpuinfo_parses_empty() {
    let runner = FakeSourceRunner::new().with_file("/proc/cpuinfo", "");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = CpuProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    assert_eq!(
        warning_pairs(&result),
        vec![
            (Some("/proc/cpuinfo"), "source_empty"),
            (Some("dmidecode -t 4"), "source_missing"),
            (Some("lscpu"), "source_missing"),
            (Some("lshw -class processor"), "source_missing"),
        ]
    );
}

#[tokio::test]
async fn cpu_probe_normalizes_vendor_from_proc_cpuinfo_samples() {
    for (path, expected_vendor, expected_architecture) in [
        ("cpu/proc-cpuinfo-intel-x86_64.txt", "Intel", None),
        ("cpu/proc-cpuinfo-amd-x86_64.txt", "AMD", None),
        ("cpu/proc-cpuinfo-hygon.txt", "Hygon", None),
        ("cpu/proc-cpuinfo-zhaoxin.txt", "Zhaoxin", None),
        (
            "cpu/proc-cpuinfo-phytium-arm64.txt",
            "Phytium",
            Some("aarch64"),
        ),
        (
            "cpu/proc-cpuinfo-kunpeng-arm64.txt",
            "HiSilicon",
            Some("aarch64"),
        ),
        (
            "cpu/proc-cpuinfo-hisilicon-kirin.txt",
            "HiSilicon",
            Some("aarch64"),
        ),
        ("cpu/proc-cpuinfo-sunway.txt", "Sunway", Some("sw_64")),
    ] {
        let runner = FakeSourceRunner::new().with_file("/proc/cpuinfo", hw_testdata::fixture(path));
        let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
        let result = CpuProbe.probe(&ctx).await;

        assert_eq!(result.devices.len(), 1, "{path}");
        let DeviceProperties::Cpu(cpu) = &result.devices[0].properties else {
            panic!("expected CPU properties");
        };
        assert_eq!(cpu.vendor.as_deref(), Some(expected_vendor), "{path}");
        assert_eq!(cpu.architecture.as_deref(), expected_architecture, "{path}");
        assert_eq!(cpu.threads, Some(2), "{path}");
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
async fn cpu_probe_warns_when_proc_hardware_parses_empty() {
    let runner = FakeSourceRunner::new()
        .with_file("/proc/cpuinfo", "")
        .with_file("/proc/hardware", "");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = CpuProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    assert_eq!(
        warning_pairs(&result),
        vec![
            (Some("/proc/cpuinfo"), "source_empty"),
            (Some("/proc/hardware"), "source_empty"),
            (Some("dmidecode -t 4"), "source_missing"),
            (Some("lscpu"), "source_missing"),
            (Some("lshw -class processor"), "source_missing"),
        ]
    );
}

#[tokio::test]
async fn cpu_probe_uses_cpufreq_sysfs_when_lscpu_frequency_fields_are_missing() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lscpu",
            std::iter::empty::<&str>(),
            "Architecture: aarch64\n\
             CPU(s): 8\n\
             Model name: Phytium D2000/8\n\
             Vendor ID: \n\
             Core(s) per socket: 8\n\
             Socket(s): 1\n",
        )
        .with_file(
            "/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq",
            "2300000\n",
        )
        .with_file(
            "/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_min_freq",
            "800000\n",
        )
        .with_file(
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq",
            "1800000\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.devices[0].sources.iter().any(|source| {
        source.kind == SourceKind::Sysfs && source.source == "/sys/devices/system/cpu/cpu0/cpufreq"
    }));
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.max_freq_mhz, Some(2300));
            assert_eq!(cpu.min_freq_mhz, Some(800));
            assert_eq!(cpu.current_freq_mhz, Some(1800));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_uses_scaling_cpufreq_limits_when_cpuinfo_limits_are_missing() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lscpu",
            std::iter::empty::<&str>(),
            "Architecture: aarch64\n\
             CPU(s): 8\n\
             Model name: Phytium D2000/8\n\
             Vendor ID: \n\
             Core(s) per socket: 8\n\
             Socket(s): 1\n",
        )
        .with_file(
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq",
            "2300000\n",
        )
        .with_file(
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_min_freq",
            "800000\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.max_freq_mhz, Some(2300));
            assert_eq!(cpu.min_freq_mhz, Some(800));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_uses_average_cpufreq_current_frequency() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lscpu",
            std::iter::empty::<&str>(),
            "Architecture: x86_64\n\
             CPU(s): 2\n\
             Model name: Intel(R) Core(TM) i7\n\
             Vendor ID: GenuineIntel\n\
             Core(s) per socket: 2\n\
             Socket(s): 1\n",
        )
        .with_glob(
            "/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq",
            vec![
                PathBuf::from("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq"),
                PathBuf::from("/sys/devices/system/cpu/cpu1/cpufreq/scaling_cur_freq"),
            ],
        )
        .with_file(
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq",
            "1600000\n",
        )
        .with_file(
            "/sys/devices/system/cpu/cpu1/cpufreq/scaling_cur_freq",
            "2200000\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.current_freq_mhz, Some(1900));
        }
        other => panic!("expected cpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn cpu_probe_uses_cpu_sysfs_topology_and_cache_when_command_sources_are_missing() {
    let cpu_paths = (0..4)
        .map(|index| PathBuf::from(format!("/sys/devices/system/cpu/cpu{index}")))
        .collect::<Vec<_>>();
    let cache_paths = (0..5)
        .map(|index| PathBuf::from(format!("/sys/devices/system/cpu/cpu0/cache/index{index}")))
        .collect::<Vec<_>>();
    let runner = FakeSourceRunner::new()
        .with_glob("/sys/devices/system/cpu/cpu*", cpu_paths)
        .with_file("/sys/devices/system/cpu/online", "0,2\n")
        .with_file(
            "/sys/devices/system/cpu/cpu0/topology/physical_package_id",
            "0\n",
        )
        .with_file("/sys/devices/system/cpu/cpu0/topology/core_id", "0\n")
        .with_file(
            "/sys/devices/system/cpu/cpu0/topology/thread_siblings_list",
            "0,2\n",
        )
        .with_file(
            "/sys/devices/system/cpu/cpu1/topology/physical_package_id",
            "0\n",
        )
        .with_file("/sys/devices/system/cpu/cpu1/topology/core_id", "1\n")
        .with_file(
            "/sys/devices/system/cpu/cpu1/topology/thread_siblings_list",
            "1,3\n",
        )
        .with_file(
            "/sys/devices/system/cpu/cpu2/topology/physical_package_id",
            "0\n",
        )
        .with_file("/sys/devices/system/cpu/cpu2/topology/core_id", "0\n")
        .with_file(
            "/sys/devices/system/cpu/cpu2/topology/thread_siblings_list",
            "0,2\n",
        )
        .with_file(
            "/sys/devices/system/cpu/cpu3/topology/physical_package_id",
            "0\n",
        )
        .with_file("/sys/devices/system/cpu/cpu3/topology/core_id", "1\n")
        .with_file(
            "/sys/devices/system/cpu/cpu3/topology/thread_siblings_list",
            "1,3\n",
        )
        .with_glob("/sys/devices/system/cpu/cpu0/cache/index*", cache_paths)
        .with_file("/sys/devices/system/cpu/cpu0/cache/index0/level", "1\n")
        .with_file("/sys/devices/system/cpu/cpu0/cache/index0/type", "Data\n")
        .with_file("/sys/devices/system/cpu/cpu0/cache/index0/size", "32K\n")
        .with_file(
            "/sys/devices/system/cpu/cpu0/cache/index0/shared_cpu_list",
            "0,2\n",
        )
        .with_file("/sys/devices/system/cpu/cpu0/cache/index1/level", "1\n")
        .with_file(
            "/sys/devices/system/cpu/cpu0/cache/index1/type",
            "Instruction\n",
        )
        .with_file("/sys/devices/system/cpu/cpu0/cache/index1/size", "32K\n")
        .with_file(
            "/sys/devices/system/cpu/cpu0/cache/index1/shared_cpu_list",
            "0,2\n",
        )
        .with_file("/sys/devices/system/cpu/cpu0/cache/index2/level", "2\n")
        .with_file(
            "/sys/devices/system/cpu/cpu0/cache/index2/type",
            "Unified\n",
        )
        .with_file("/sys/devices/system/cpu/cpu0/cache/index2/size", "256K\n")
        .with_file(
            "/sys/devices/system/cpu/cpu0/cache/index2/shared_cpu_list",
            "0,2\n",
        )
        .with_file("/sys/devices/system/cpu/cpu0/cache/index3/level", "3\n")
        .with_file(
            "/sys/devices/system/cpu/cpu0/cache/index3/type",
            "Unified\n",
        )
        .with_file("/sys/devices/system/cpu/cpu0/cache/index3/size", "8M\n")
        .with_file(
            "/sys/devices/system/cpu/cpu0/cache/index3/shared_cpu_list",
            "0-3\n",
        )
        .with_file("/sys/devices/system/cpu/cpu0/cache/index4/level", "4\n")
        .with_file(
            "/sys/devices/system/cpu/cpu0/cache/index4/type",
            "Unified\n",
        )
        .with_file("/sys/devices/system/cpu/cpu0/cache/index4/size", "16M\n")
        .with_file(
            "/sys/devices/system/cpu/cpu0/cache/index4/shared_cpu_list",
            "0-3\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = CpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_source_status(
        &result.devices[0],
        "/sys/devices/system/cpu",
        SourceStatus::Success,
    );
    match &result.devices[0].properties {
        DeviceProperties::Cpu(cpu) => {
            assert_eq!(cpu.threads, Some(4));
            assert_eq!(cpu.online_threads, Some(2));
            assert_eq!(cpu.online_cores, Some(1));
            assert_eq!(cpu.cores, Some(2));
            assert_eq!(cpu.threads_per_core, Some(2));
            assert_eq!(cpu.sockets, Some(1));
            assert_eq!(cpu.l1d_cache.as_deref(), Some("64 KiB"));
            assert_eq!(cpu.l1i_cache.as_deref(), Some("64 KiB"));
            assert_eq!(cpu.l2_cache.as_deref(), Some("512 KiB"));
            assert_eq!(cpu.l3_cache.as_deref(), Some("8 MiB"));
            assert_eq!(cpu.l4_cache.as_deref(), Some("16 MiB"));
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
async fn network_probe_preserves_pci_identity_and_modules_from_sysfs() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "ip",
            ["-j", "link"],
            r#"[{"ifname":"enp1s0","address":"aa:bb:cc:dd:ee:ff","operstate":"UP","mtu":1500}]"#,
        )
        .with_command("ip", ["-j", "addr"], "[]")
        .with_file(
            "/sys/class/net/enp1s0/device/uevent",
            "DRIVER=e1000e\nPCI_CLASS=20000\nPCI_ID=8086:15F3\nPCI_SUBSYS_ID=8086:0000\nPCI_SLOT_NAME=0000:01:00.0\n",
        )
        .with_glob(
            "/sys/class/net/enp1s0/device/driver/module/drivers/*",
            vec![PathBuf::from(
                "/sys/class/net/enp1s0/device/driver/module/drivers/pci:e1000e",
            )],
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = NetworkProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(
        device.bus,
        Some(BusInfo::Pci {
            address: "0000:01:00.0".to_string(),
            vendor_id: Some("8086".to_string()),
            device_id: Some("15f3".to_string()),
            subsystem_vendor_id: Some("8086".to_string()),
            subsystem_device_id: Some("0000".to_string()),
            class: Some("020000".to_string()),
        })
    );
    assert_eq!(
        device
            .driver
            .as_ref()
            .map(|driver| driver.modules.as_slice()),
        Some(&["e1000e".to_string()][..])
    );
}

#[tokio::test]
async fn network_probe_enriches_human_readable_lshw_fields() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "ip",
            ["-j", "link"],
            r#"[{"ifname":"enp0s31f6","address":"aa:bb:cc:dd:ee:ff","operstate":"UP","mtu":1500}]"#,
        )
        .with_command("ip", ["-j", "addr"], "[]")
        .with_command(
            "lshw",
            ["-class", "network"],
            "  *-network\n\
                  description: Ethernet interface\n\
                  product: Ethernet Connection (16) I219-LM\n\
                  vendor: Intel Corporation\n\
                  bus info: pci@0000:00:1f.6\n\
                  logical name: enp0s31f6\n\
                  serial: aa:bb:cc:dd:ee:ff\n\
                  capacity: 1Gbit/s\n\
                  configuration: broadcast=yes driver=e1000e driverversion=6.8.0 firmware=0.8-4\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = NetworkProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.vendor.as_deref(), Some("Intel Corporation"));
    assert_eq!(
        device.model.as_deref(),
        Some("Ethernet Connection (16) I219-LM")
    );
    assert_eq!(
        device.bus,
        Some(BusInfo::Pci {
            address: "0000:00:1f.6".to_string(),
            vendor_id: None,
            device_id: None,
            subsystem_vendor_id: None,
            subsystem_device_id: None,
            class: None,
        })
    );
    assert!(
        device
            .sources
            .iter()
            .any(|source| source.kind == SourceKind::Command
                && source.source == "lshw -class network")
    );
    assert_eq!(
        device
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("e1000e")
    );
    assert_eq!(
        device
            .driver
            .as_ref()
            .and_then(|driver| driver.version.as_deref()),
        Some("6.8.0")
    );
    match &device.properties {
        DeviceProperties::Network(network) => {
            assert_eq!(network.speed_mbps, Some(1000));
            assert_eq!(network.firmware.as_deref(), Some("0.8-4"));
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
async fn storage_probe_reports_deepin_size_display_rounding() {
    let cases = [
        (512_110_190_592_u64, "512 GB", "sata"),
        (1_000_204_886_016_u64, "1 TB", "nvme"),
        (16_031_383_552_u64, "16 GB", "usb"),
    ];

    for (size, expected, tran) in cases {
        let runner = FakeSourceRunner::new().with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            format!(
                r#"{{"blockdevices":[{{"name":"sda","type":"disk","size":{size},"model":"Disk","serial":"S1","tran":"{tran}"}}]}}"#
            ),
        );
        let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

        let result = StorageProbe.probe(&ctx).await;

        let DeviceProperties::Storage(storage) = &result.devices[0].properties else {
            panic!("expected storage properties");
        };
        assert_eq!(storage.size_bytes, Some(size));
        assert_eq!(storage.size_display.as_deref(), Some(expected), "{tran}");
    }
}

#[tokio::test]
async fn storage_probe_infers_longsys_vendor_from_rs_model_prefix() {
    let runner = FakeSourceRunner::new().with_command(
        "lsblk",
        ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
        r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"RSYE3836N-480G","serial":"S1","tran":"sata"}]}"#,
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].vendor.as_deref(), Some("Longsys"));
    assert_eq!(result.devices[0].model.as_deref(), Some("RSYE3836N-480G"));
}

#[tokio::test]
async fn storage_probe_infers_vendor_from_kylin_disk_model_prefix() {
    let cases = [
        ("WDC WD10EZEX-08WN4A0", "Western Digital"),
        ("ST1000DM010-2EP102", "Seagate"),
        ("FORESEE 256GB SSD", "Foresee"),
        ("YMTC PC300 512GB", "YMTC"),
        ("ZHITAI TiPlus5000", "ZhiTai"),
        ("ZTC ZTSSD-A4", "ZhiTai"),
        ("YEESTOR YS9082", "Yeestor"),
        ("MAXIO MAP1202", "Maxio"),
        ("GLOWAY VAL 512G", "Gloway"),
        ("KingSpec NT-256", "KingSpec"),
        ("KINGSTON SA400S37", "Kingston"),
        ("SanDisk SSD PLUS", "SanDisk"),
        ("SAMSUNG MZVL2512HCJQ", "Samsung"),
        ("MICRON_2450_MTFDKBA512TFK", "Micron"),
        ("CT500MX500SSD1", "Crucial"),
        ("SKHynix_HFS512GDE9X081N", "SK hynix"),
        ("HYNIX HFS256G39TND", "SK hynix"),
        ("NETAC SSD 512GB", "Netac"),
        ("RAMAXEL RPEYJ1T24MKN2QW", "Ramaxel"),
        ("BIWIN SSD", "Biwin"),
        ("CXMT SSD", "CXMT"),
        ("TIGO SSD", "Tigo"),
        ("Colorful CN600", "Colorful"),
        ("Asgard AN4", "Asgard"),
        ("LEXAR SSD NM620", "Lexar"),
    ];

    for (model, vendor) in cases {
        let runner = FakeSourceRunner::new().with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            format!(
                r#"{{"blockdevices":[{{"name":"sda","type":"disk","size":1024,"model":"{model}","serial":"S1","tran":"sata"}}]}}"#
            ),
        );
        let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

        let result = StorageProbe.probe(&ctx).await;

        assert_eq!(result.devices.len(), 1, "{model}");
        assert_eq!(result.devices[0].vendor.as_deref(), Some(vendor), "{model}");
        assert_eq!(result.devices[0].model.as_deref(), Some(model), "{model}");
    }
}

#[tokio::test]
async fn storage_probe_does_not_infer_vendor_from_kernel_name_without_model() {
    let runner = FakeSourceRunner::new().with_command(
        "lsblk",
        ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
        r#"{"blockdevices":[{"name":"st0","type":"disk","size":1024,"serial":"S1","tran":"sata"}]}"#,
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].vendor, None);
    assert_eq!(result.devices[0].model, None);
}

#[tokio::test]
async fn storage_probe_keeps_explicit_vendor_over_model_prefix_fallback() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"WDC WD10EZEX-08WN4A0","serial":"S1","tran":"sata"}]}"#,
        )
        .with_command(
            "lshw",
            ["-class", "disk"],
            "  *-disk\n\
                  description: ATA Disk\n\
                  product: WDC WD10EZEX-08WN4A0\n\
                  vendor: ACME Storage\n\
                  logical name: /dev/sda\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].vendor.as_deref(), Some("ACME Storage"));
    assert_eq!(
        result.devices[0].model.as_deref(),
        Some("WDC WD10EZEX-08WN4A0")
    );
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
async fn storage_probe_enriches_identity_from_smartctl() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"USB SATA Bridge","tran":"sata"}]}"#,
        )
        .with_command(
            "smartctl",
            ["-a", "-j", "/dev/sda"],
            r#"{
              "model_name": "Samsung SSD 870 EVO 500GB",
              "serial_number": "S6P012345678",
              "firmware_version": "SVT02B6Q"
            }"#,
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices[0].id, "storage:serial:S6P012345678");
    assert_eq!(result.devices[0].name, "Samsung SSD 870 EVO 500GB");
    assert_eq!(
        result.devices[0].model.as_deref(),
        Some("Samsung SSD 870 EVO 500GB")
    );
    assert_eq!(result.devices[0].serial.as_deref(), Some("S6P012345678"));
    let DeviceProperties::Storage(storage) = &result.devices[0].properties else {
        panic!("expected storage properties");
    };
    assert_eq!(storage.firmware.as_deref(), Some("SVT02B6Q"));
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command
            && source.source == "smartctl -a -j /dev/sda"));
}

#[tokio::test]
async fn storage_probe_reads_nvme_smart_health_details() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"nvme0n1","type":"disk","size":1024,"model":"NVMe SSD","serial":"N1","tran":"nvme"}]}"#,
        )
        .with_command(
            "smartctl",
            ["-a", "-j", "/dev/nvme0n1"],
            r#"{
              "smart_status": {"passed": true},
              "temperature": {"current": 37},
              "power_on_time": {"hours": 1234},
              "power_cycle_count": 56,
              "nvme_smart_health_information_log": {
                "available_spare": 99,
                "available_spare_threshold": 10,
                "percentage_used": 3,
                "data_units_read": 123456,
                "data_units_written": 654321,
                "media_errors": 2,
                "num_err_log_entries": 4
              }
            }"#,
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let DeviceProperties::Storage(storage) = &result.devices[0].properties else {
        panic!("expected storage properties");
    };
    assert_eq!(storage.power_on_hours, Some(1234));
    assert_eq!(storage.power_cycle_count, Some(56));
    assert_eq!(storage.available_spare_percent, Some(99));
    assert_eq!(storage.available_spare_threshold_percent, Some(10));
    assert_eq!(storage.percentage_used, Some(3));
    assert_eq!(storage.data_units_read, Some(123456));
    assert_eq!(storage.data_units_written, Some(654321));
    assert_eq!(storage.media_errors, Some(2));
    assert_eq!(storage.error_log_entries, Some(4));
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command
            && source.source == "smartctl -a -j /dev/nvme0n1"));
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
async fn storage_probe_retries_usb_bridge_smartctl_with_sat_device_type() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"USB SSD","serial":"S1","tran":"usb"}]}"#,
        )
        .with_command_status(
            "smartctl",
            ["-a", "-j", "/dev/sda"],
            "Read Device Identity failed: scsi error unsupported scsi opcode\n",
            2,
        )
        .with_command(
            "smartctl",
            ["-a", "-j", "-d", "sat", "/dev/sda"],
            r#"{"smart_status":{"passed":true},"temperature":{"current":39}}"#,
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = StorageProbe.probe(&ctx).await;

    let DeviceProperties::Storage(storage) = &result.devices[0].properties else {
        panic!("expected storage properties");
    };
    assert_eq!(storage.smart_status.as_deref(), Some("passed"));
    assert_eq!(storage.temperature_celsius, Some(39.0));
    assert!(result.devices[0].sources.iter().any(|source| {
        source.kind == SourceKind::Command && source.source == "smartctl -a -j -d sat /dev/sda"
    }));
}

#[tokio::test]
async fn storage_probe_records_warning_when_smartctl_json_is_malformed() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"Disk","serial":"S1","tran":"sata"}]}"#,
        )
        .with_command("smartctl", ["-a", "-j", "/dev/sda"], "not json");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.devices[0].warnings.iter().any(|warning| {
        warning.code == "parse_failed"
            && warning.source.as_deref() == Some("smartctl -a -j /dev/sda")
    }));
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
async fn storage_probe_preserves_sata_parent_pci_identity_from_sysfs() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"SATA SSD","serial":"S1","tran":"sata"}]}"#,
        )
        .with_file("/sys/block/sda/device/uevent", "DRIVER=sd\n")
        .with_file(
            "/sys/block/sda/device/../../../uevent",
            "DRIVER=ahci\nPCI_CLASS=10601\nPCI_ID=8086:A352\nPCI_SUBSYS_ID=1028:087C\nPCI_SLOT_NAME=0000:00:17.0\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(
        device.bus,
        Some(BusInfo::Pci {
            address: "0000:00:17.0".to_string(),
            vendor_id: Some("8086".to_string()),
            device_id: Some("a352".to_string()),
            subsystem_vendor_id: Some("1028".to_string()),
            subsystem_device_id: Some("087c".to_string()),
            class: Some("010601".to_string()),
        })
    );
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Sysfs
            && source.source == "/sys/block/sda/device/../../../uevent"));
    assert_eq!(device.parent_id.as_deref(), Some("pci:0000:00:17.0"));
    assert_eq!(result.consumed[0].id, "pci:0000:00:17.0");
}

#[tokio::test]
async fn storage_probe_uses_unique_sysfs_storage_controller_when_parent_uevent_is_unavailable() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"SATA SSD","serial":"S1","tran":"sata"}]}"#,
        )
        .with_file("/sys/block/sda/device/uevent", "DRIVER=sd\n")
        .with_glob(
            "/sys/bus/pci/devices/*",
            vec![PathBuf::from("/sys/bus/pci/devices/0000:00:17.0")],
        )
        .with_file(
            "/sys/bus/pci/devices/0000:00:17.0/uevent",
            "DRIVER=ahci\nPCI_CLASS=10601\nPCI_ID=8086:A352\nPCI_SUBSYS_ID=1028:087C\nPCI_SLOT_NAME=0000:00:17.0\n",
        )
        .with_file("/sys/bus/pci/devices/0000:00:17.0/vendor", "0x8086\n")
        .with_file("/sys/bus/pci/devices/0000:00:17.0/device", "0xa352\n")
        .with_file("/sys/bus/pci/devices/0000:00:17.0/class", "0x010601\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(
        device.bus,
        Some(BusInfo::Pci {
            address: "0000:00:17.0".to_string(),
            vendor_id: Some("8086".to_string()),
            device_id: Some("a352".to_string()),
            subsystem_vendor_id: Some("1028".to_string()),
            subsystem_device_id: Some("087c".to_string()),
            class: Some("010601".to_string()),
        })
    );
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Sysfs
            && source.source == "/sys/bus/pci/devices/0000:00:17.0/uevent"));
}

#[tokio::test]
async fn storage_probe_uses_unique_matching_sysfs_storage_controller() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"SATA SSD","serial":"S1","tran":"sata"}]}"#,
        )
        .with_file("/sys/block/sda/device/uevent", "DRIVER=sd\n")
        .with_glob(
            "/sys/bus/pci/devices/*",
            vec![
                PathBuf::from("/sys/bus/pci/devices/0000:00:17.0"),
                PathBuf::from("/sys/bus/pci/devices/0000:0d:00.0"),
            ],
        )
        .with_file(
            "/sys/bus/pci/devices/0000:00:17.0/uevent",
            "DRIVER=ahci\nPCI_CLASS=10601\nPCI_ID=8086:A352\nPCI_SLOT_NAME=0000:00:17.0\n",
        )
        .with_file("/sys/bus/pci/devices/0000:00:17.0/vendor", "0x8086\n")
        .with_file("/sys/bus/pci/devices/0000:00:17.0/device", "0xa352\n")
        .with_file("/sys/bus/pci/devices/0000:00:17.0/class", "0x010601\n")
        .with_file(
            "/sys/bus/pci/devices/0000:0d:00.0/uevent",
            "DRIVER=nvme\nPCI_CLASS=10802\nPCI_ID=144D:A80A\nPCI_SLOT_NAME=0000:0d:00.0\n",
        )
        .with_file("/sys/bus/pci/devices/0000:0d:00.0/vendor", "0x144d\n")
        .with_file("/sys/bus/pci/devices/0000:0d:00.0/device", "0xa80a\n")
        .with_file("/sys/bus/pci/devices/0000:0d:00.0/class", "0x010802\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(
        device.bus,
        Some(BusInfo::Pci {
            address: "0000:00:17.0".to_string(),
            vendor_id: Some("8086".to_string()),
            device_id: Some("a352".to_string()),
            subsystem_vendor_id: None,
            subsystem_device_id: None,
            class: Some("010601".to_string()),
        })
    );
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Sysfs
            && source.source == "/sys/bus/pci/devices/0000:00:17.0/uevent"));
}

#[tokio::test]
async fn storage_probe_uses_sysfs_device_path_pci_ancestor_for_same_media_controllers() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"SATA SSD","serial":"S1","tran":"sata"}]}"#,
        )
        .with_file("/sys/block/sda/device/uevent", "DRIVER=sd\n")
        .with_canonical_path(
            "/sys/block/sda/device",
            "/sys/devices/pci0000:00/0000:00:1c.0/0000:03:00.0/ata1/host0/target0:0:0/0:0:0:0",
        )
        .with_glob(
            "/sys/bus/pci/devices/*",
            vec![
                PathBuf::from("/sys/bus/pci/devices/0000:03:00.0"),
                PathBuf::from("/sys/bus/pci/devices/0000:00:1f.2"),
            ],
        )
        .with_file(
            "/sys/bus/pci/devices/0000:03:00.0/uevent",
            "DRIVER=ahci\nPCI_CLASS=10601\nPCI_ID=8086:A352\nPCI_SLOT_NAME=0000:03:00.0\n",
        )
        .with_file("/sys/bus/pci/devices/0000:03:00.0/vendor", "0x8086\n")
        .with_file("/sys/bus/pci/devices/0000:03:00.0/device", "0xa352\n")
        .with_file("/sys/bus/pci/devices/0000:03:00.0/class", "0x010601\n")
        .with_file(
            "/sys/bus/pci/devices/0000:00:1f.2/uevent",
            "DRIVER=ahci\nPCI_CLASS=10601\nPCI_ID=8086:2922\nPCI_SLOT_NAME=0000:00:1f.2\n",
        )
        .with_file("/sys/bus/pci/devices/0000:00:1f.2/vendor", "0x8086\n")
        .with_file("/sys/bus/pci/devices/0000:00:1f.2/device", "0x2922\n")
        .with_file("/sys/bus/pci/devices/0000:00:1f.2/class", "0x010601\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(
        device.bus,
        Some(BusInfo::Pci {
            address: "0000:03:00.0".to_string(),
            vendor_id: Some("8086".to_string()),
            device_id: Some("a352".to_string()),
            subsystem_vendor_id: None,
            subsystem_device_id: None,
            class: Some("010601".to_string()),
        })
    );
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Sysfs
            && source.source == "/sys/bus/pci/devices/0000:03:00.0/uevent"));
}

#[tokio::test]
async fn storage_probe_preserves_nvme_controller_pci_identity_from_sysfs() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"nvme0n1","type":"disk","size":1024,"model":"NVMe Disk","serial":"N1","tran":"nvme"}]}"#,
        )
        .with_file(
            "/sys/class/nvme/nvme0/device/uevent",
            "DRIVER=nvme\nPCI_CLASS=10802\nPCI_ID=144D:A80A\nPCI_SUBSYS_ID=144D:A801\nPCI_SLOT_NAME=0000:0d:00.0\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(
        device.bus,
        Some(BusInfo::Pci {
            address: "0000:0d:00.0".to_string(),
            vendor_id: Some("144d".to_string()),
            device_id: Some("a80a".to_string()),
            subsystem_vendor_id: Some("144d".to_string()),
            subsystem_device_id: Some("a801".to_string()),
            class: Some("010802".to_string()),
        })
    );
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Sysfs
            && source.source == "/sys/class/nvme/nvme0/device/uevent"));
}

#[tokio::test]
async fn storage_probe_enriches_controller_identity_from_lshw_storage() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"nvme0n1","type":"disk","size":1024,"model":"NVMe Disk","serial":"N1","tran":"nvme"}]}"#,
        )
        .with_file(
            "/sys/class/nvme/nvme0/device/uevent",
            "DRIVER=nvme\nPCI_CLASS=10802\nPCI_ID=144D:A80A\nPCI_SUBSYS_ID=144D:A801\nPCI_SLOT_NAME=0000:0d:00.0\n",
        )
        .with_command(
            "lshw",
            ["-class", "storage"],
            "  *-storage\n\
                  description: Non-Volatile memory controller\n\
                  product: NVMe SSD Controller PM9A1/PM9A3/980PRO\n\
                  vendor: Samsung Electronics Co Ltd\n\
                  bus info: pci@0000:0d:00.0\n\
                  configuration: driver=nvme latency=0\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let device = &result.devices[0];
    let DeviceProperties::Storage(storage) = &device.properties else {
        panic!("expected storage properties");
    };
    assert_eq!(
        storage.controller_vendor.as_deref(),
        Some("Samsung Electronics Co Ltd")
    );
    assert_eq!(
        storage.controller_model.as_deref(),
        Some("NVMe SSD Controller PM9A1/PM9A3/980PRO")
    );
    assert_eq!(storage.controller_driver.as_deref(), Some("nvme"));
    assert_eq!(device.parent_id.as_deref(), Some("pci:0000:0d:00.0"));
    assert_eq!(result.consumed[0].id, "pci:0000:0d:00.0");
    assert!(device.sources.iter().any(|source| {
        source.kind == SourceKind::Command && source.source == "lshw -class storage"
    }));
}

#[tokio::test]
async fn storage_probe_enriches_controller_identity_from_lspci_when_lshw_storage_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"SATA SSD","serial":"S1","tran":"sata"}]}"#,
        )
        .with_file("/sys/block/sda/device/uevent", "DRIVER=sd\n")
        .with_file(
            "/sys/block/sda/device/../../../uevent",
            "DRIVER=ahci\nPCI_CLASS=10601\nPCI_ID=8086:A352\nPCI_SLOT_NAME=0000:00:17.0\n",
        )
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "00:17.0 SATA controller [0106]: Intel Corporation Cannon Lake Mobile PCH SATA AHCI Controller [8086:a352]\n\tKernel driver in use: ahci\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let device = &result.devices[0];
    let DeviceProperties::Storage(storage) = &device.properties else {
        panic!("expected storage properties");
    };
    assert_eq!(
        storage.controller_model.as_deref(),
        Some("Intel Corporation Cannon Lake Mobile PCH SATA AHCI Controller")
    );
    assert_eq!(storage.controller_driver.as_deref(), Some("ahci"));
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "lspci -nn -k"));
}

#[tokio::test]
async fn storage_probe_enriches_human_readable_lshw_fields() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"tran":"sata"}]}"#,
        )
        .with_command(
            "lshw",
            ["-class", "disk"],
            "  *-disk\n\
                  description: ATA Disk\n\
                  product: Samsung SSD 980\n\
                  vendor: Samsung\n\
                  logical name: /dev/sda\n\
                  serial: S12345\n\
                  capabilities: gpt-1.00 partitioned partitioned:gpt\n\
                  configuration: ansiversion=5 firmware=3B2QGXA7 sectorsize=512 speed=6Gbit/s\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.name, "Samsung SSD 980");
    assert_eq!(device.vendor.as_deref(), Some("Samsung"));
    assert_eq!(device.model.as_deref(), Some("Samsung SSD 980"));
    assert_eq!(device.serial.as_deref(), Some("S12345"));
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "lshw -class disk"));
    let DeviceProperties::Storage(storage) = &device.properties else {
        panic!("expected storage properties");
    };
    assert_eq!(storage.firmware.as_deref(), Some("3B2QGXA7"));
    assert_eq!(storage.speed.as_deref(), Some("6Gbit/s"));
    assert_eq!(
        storage.capabilities,
        vec!["gpt-1.00", "partitioned", "partitioned:gpt"]
    );
}

#[tokio::test]
async fn storage_probe_enriches_human_readable_hwinfo_fields() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"nvme0n1","type":"disk","size":1024,"tran":"nvme"}]}"#,
        )
        .with_command(
            "hwinfo",
            ["--disk"],
            "30: IDE 00.0: 10600 Disk\n\
                 Hardware Class: disk\n\
                 Model: \"Samsung SSD 980\"\n\
                 Vendor: \"Samsung\"\n\
                 Revision: \"3B2QGXA7\"\n\
                 Driver: \"nvme\"\n\
                 Driver Modules: \"nvme\"\n\
                 Device File: /dev/nvme0n1\n\
                 Serial ID: \"S12345\"\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.name, "Samsung SSD 980");
    assert_eq!(device.vendor.as_deref(), Some("Samsung"));
    assert_eq!(device.model.as_deref(), Some("Samsung SSD 980"));
    assert_eq!(device.serial.as_deref(), Some("S12345"));
    assert_eq!(
        device
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("nvme")
    );
    assert_eq!(
        device
            .driver
            .as_ref()
            .map(|driver| driver.modules.as_slice()),
        Some(&["nvme".to_string()][..])
    );
    assert!(device
        .sources
        .iter()
        .any(|source| { source.kind == SourceKind::Command && source.source == "hwinfo --disk" }));
    let DeviceProperties::Storage(storage) = &device.properties else {
        panic!("expected storage properties");
    };
    assert_eq!(storage.firmware.as_deref(), Some("3B2QGXA7"));
}

#[tokio::test]
async fn storage_probe_enriches_hdparm_identity_fields() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"tran":"sata"}]}"#,
        )
        .with_command(
            "hdparm",
            ["-i", "/dev/sda"],
            "/dev/sda:\n\
             \n\
             Model=Samsung SSD 870 EVO 500GB, FwRev=SVT02B6Q, SerialNo=S6P012345678\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.name, "Samsung SSD 870 EVO 500GB");
    assert_eq!(device.model.as_deref(), Some("Samsung SSD 870 EVO 500GB"));
    assert_eq!(device.serial.as_deref(), Some("S6P012345678"));
    assert!(device.sources.iter().any(|source| {
        source.kind == SourceKind::Command && source.source == "hdparm -i /dev/sda"
    }));
    let DeviceProperties::Storage(storage) = &device.properties else {
        panic!("expected storage properties");
    };
    assert_eq!(storage.firmware.as_deref(), Some("SVT02B6Q"));
}

#[tokio::test]
async fn storage_probe_uses_hwinfo_when_lsblk_and_sysfs_are_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "hwinfo",
        ["--disk"],
        "30: IDE 00.0: 10600 Disk\n\
             Hardware Class: disk\n\
             Model: \"Samsung SSD 980\"\n\
             Vendor: \"Samsung\"\n\
             Revision: \"3B2QGXA7\"\n\
             Driver: \"nvme\"\n\
             Device File: /dev/nvme0n1\n\
             Serial ID: \"S12345\"\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    let device = &result.devices[0];
    assert_eq!(device.id, "storage:serial:S12345");
    assert_eq!(device.name, "Samsung SSD 980");
    assert_eq!(device.vendor.as_deref(), Some("Samsung"));
    assert_eq!(device.model.as_deref(), Some("Samsung SSD 980"));
    assert_eq!(device.serial.as_deref(), Some("S12345"));
    let DeviceProperties::Storage(storage) = &device.properties else {
        panic!("expected storage properties");
    };
    assert_eq!(storage.device_node.as_deref(), Some("/dev/nvme0n1"));
    assert_eq!(storage.firmware.as_deref(), Some("3B2QGXA7"));
    assert!(device
        .sources
        .iter()
        .any(|source| { source.kind == SourceKind::Command && source.source == "hwinfo --disk" }));
    assert!(result.warnings.iter().any(|warning| {
        warning.code == "source_missing"
            && warning.source.as_deref()
                == Some("lsblk -J -b -o NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV")
    }));
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
        .with_file(
            "/sys/block/sda/device/uevent",
            "DRIVER=sd\nPRODUCT=abcd/1234/0001\n",
        )
        .with_file(
            "/sys/block/sda/device/../uevent",
            "DRIVER=ufs\nPCI_CLASS=180000\nPCI_ID=1D87:0001\nPCI_SLOT_NAME=0000:01:00.0\n",
        )
        .with_file("/sys/block/sda/device/spec_version", "3.1\n")
        .with_file("/sys/block/sda/device/modalias", "scsi:t-0x00\n")
        .with_file("/sys/block/sda/device/unique_number", "UNIQUE-S12345\n")
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
            assert_eq!(storage.media_type.as_deref(), Some("ufs"));
            assert_eq!(storage.rotation_rate.as_deref(), Some("Solid State Device"));
            assert_eq!(storage.ufs_spec_version.as_deref(), Some("3.1"));
            assert_eq!(storage.vid_pid.as_deref(), Some("abcd/1234/0001"));
            assert_eq!(storage.phys_id.as_deref(), Some("abcd/1234/0001"));
            assert_eq!(storage.modalias.as_deref(), Some("scsi:t-0x00"));
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
    assert_eq!(
        result.devices[0].parent_id.as_deref(),
        Some("pci:0000:01:00.0")
    );
    assert_eq!(result.consumed[0].id, "pci:0000:01:00.0");
}

#[tokio::test]
async fn storage_probe_prefers_bootdevice_cid_for_platform_disk_serial() {
    let runner = FakeSourceRunner::new()
        .with_glob("/sys/block/*", vec![PathBuf::from("/sys/block/sda")])
        .with_file("/sys/block/sda/device/model", "UFS Disk\n")
        .with_file("/sys/block/sda/device/name", "f8300000.ufs\n")
        .with_file("/sys/block/sda/device/spec_version", "3.1\n")
        .with_file("/proc/bootdevice/name", "f8300000.ufs\n")
        .with_file("/proc/bootdevice/cid", "CID-SERIAL-123\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "storage:serial:CID-SERIAL-123");
    assert_eq!(result.devices[0].serial.as_deref(), Some("CID-SERIAL-123"));
}

#[tokio::test]
async fn storage_probe_uses_sysfs_when_lsblk_parses_no_disks() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lsblk",
            ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"],
            r#"{"blockdevices":[]}"#,
        )
        .with_glob("/sys/block/*", vec![PathBuf::from("/sys/block/sda")])
        .with_file("/sys/block/sda/size", "2097152\n")
        .with_file("/sys/block/sda/device/vendor", "Samsung\n")
        .with_file("/sys/block/sda/device/model", "Samsung SSD 980\n")
        .with_file("/sys/block/sda/device/serial", "S12345\n")
        .with_file("/sys/block/sda/queue/rotational", "0\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = StorageProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "storage:serial:S12345");
    assert_eq!(result.devices[0].vendor.as_deref(), Some("Samsung"));
    assert_eq!(result.devices[0].model.as_deref(), Some("Samsung SSD 980"));
    assert_eq!(
        warning_pairs(&result),
        vec![(
            Some("lsblk -J -b -o NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV"),
            "source_empty"
        )]
    );
    match &result.devices[0].properties {
        DeviceProperties::Storage(storage) => {
            assert_eq!(storage.device_node.as_deref(), Some("/dev/sda"));
            assert_eq!(storage.size_bytes, Some(1_073_741_824));
            assert_eq!(storage.media_type.as_deref(), Some("ssd"));
        }
        other => panic!("expected storage properties, got {other:?}"),
    }
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
