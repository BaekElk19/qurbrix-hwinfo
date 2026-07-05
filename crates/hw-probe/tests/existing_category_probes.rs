use hw_model::DeviceKind;
use hw_probe::{CpuProbe, NetworkProbe, Probe, ProbeContext, StorageProbe};
use hw_source::FakeSourceRunner;
use std::time::Duration;

#[tokio::test]
async fn cpu_probe_outputs_cpu_device() {
    let runner = FakeSourceRunner::new().with_command(
        "lscpu",
        std::iter::empty::<&str>(),
        "Architecture: x86_64\nCPU(s): 8\nModel name: AMD Ryzen 7\nVendor ID: AuthenticAMD\nCore(s) per socket: 4\nSocket(s): 1\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = CpuProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Cpu);
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
