use async_trait::async_trait;
use hw_model::{Device, DeviceKind, DeviceProperties, SourceStatus};
use hw_probe::{CpuProbe, NetworkProbe, Probe, ProbeContext, StorageProbe};
use hw_source::{
    CommandSpec, FakeSourceRunner, GlobResult, SourceBytesResult, SourceErrorKind, SourceResult,
    SourceRunner,
};
use std::{path::Path, time::Duration};

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
            "Architecture: x86_64\nCPU(s): 8\nModel name: AMD Ryzen 7\nVendor ID: AuthenticAMD\nCore(s) per socket: 4\nSocket(s): 1\nFlags: fpu sse sse2\n",
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
async fn network_probe_outputs_network_device() {
    let runner = FakeSourceRunner::new().with_command(
        "ip",
        ["-j", "link"],
        r#"[{"ifname":"eth0","address":"aa:bb:cc:dd:ee:ff","operstate":"UP","mtu":1500}]"#,
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = NetworkProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Network);
}

#[tokio::test]
async fn storage_probe_outputs_storage_device() {
    let runner = FakeSourceRunner::new().with_command(
        "lsblk",
        ["-J", "-b", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN"],
        r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"Disk","serial":"S1","tran":"sata"}]}"#,
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = StorageProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Storage);
}
