use hw_bindid::{collect_bindid_report_with_runner, BindIdStatus};
use hw_source::{
    CommandSpec, FakeSourceRunner, GlobResult, SourceBytesResult, SourceErrorKind, SourceResult,
    SourceRunner,
};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::test]
async fn collector_returns_failed_report_when_required_sources_are_missing() {
    let runner = FakeSourceRunner::new();
    let report = collect_bindid_report_with_runner(&runner, Duration::from_secs(1))
        .await
        .unwrap();

    assert_eq!(report.status, BindIdStatus::Failed);
    assert!(report.value.is_none());
    assert!(report
        .missing_required_kinds
        .contains(&"system".to_string()));
    assert!(report
        .missing_required_kinds
        .contains(&"motherboard".to_string()));
    assert!(report
        .missing_required_kinds
        .contains(&"memory".to_string()));
    assert!(report
        .missing_required_kinds
        .contains(&"storage".to_string()));
    assert!(report
        .missing_required_kinds
        .contains(&"network".to_string()));
    assert!(!report.warnings.is_empty());
}

#[tokio::test]
async fn collector_converts_narrow_probe_devices_into_component_keys() {
    let runner = FakeSourceRunner::new().with_command(
        "lsblk",
        [
            "-J",
            "-b",
            "-o",
            "NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV,MOUNTPOINT,FSTYPE,PARTUUID,LABEL",
        ],
        r#"{"blockdevices":[{"name":"sda","type":"disk","size":1024,"model":"Disk","serial":"S1","tran":"sata"}]}"#,
    );
    let report = collect_bindid_report_with_runner(&runner, Duration::from_secs(1))
        .await
        .unwrap();

    assert_eq!(report.status, BindIdStatus::Failed);
    assert!(report.value.is_none());
    assert_eq!(
        report.component_keys,
        vec!["storage:model=Disk|serial=S1".to_string()]
    );
    assert_eq!(report.covered_kinds, vec!["storage".to_string()]);
    assert!(!report
        .missing_required_kinds
        .contains(&"storage".to_string()));
    assert!(!report.warnings.is_empty());
}

#[tokio::test]
async fn collector_keeps_bindid_source_surface_narrow() {
    let runner = StrictRecordingRunner::new();

    let _report = collect_bindid_report_with_runner(&runner, Duration::from_secs(1))
        .await
        .unwrap();

    let calls = runner.recorded_calls();
    assert!(calls.commands.contains(&"dmidecode -t 1".to_string()));
    assert!(calls.commands.contains(&"dmidecode -t 0,1,2,3".to_string()));
    assert!(calls.commands.contains(&"dmidecode -t memory".to_string()));
    assert!(calls.commands.contains(
        &"lsblk -J -b -o NAME,TYPE,SIZE,MODEL,SERIAL,TRAN,WWN,REV,MOUNTPOINT,FSTYPE,PARTUUID,LABEL"
            .to_string()
    ));
    assert!(calls.commands.contains(&"ip -j link".to_string()));
    assert!(calls.commands.contains(&"lspci -nn -k".to_string()));
    assert!(!calls.commands.contains(&"lscpu".to_string()));
    assert!(!calls.commands.contains(&"lsusb".to_string()));
    assert!(!calls.commands.contains(&"pactl list cards".to_string()));
    assert!(!calls
        .commands
        .contains(&"bluetoothctl paired-devices".to_string()));
    assert!(!calls.commands.contains(&"upower --dump".to_string()));
    assert!(!calls
        .commands
        .contains(&"v4l2-ctl --list-devices".to_string()));
    assert!(!calls.commands.contains(&"lpstat -a".to_string()));
    assert!(!calls.commands.contains(&"xrandr --verbose".to_string()));
    assert!(!calls.commands.contains(&"hwinfo --monitor".to_string()));
    assert!(!calls.files.contains(&"/proc/cpuinfo".to_string()));
    assert!(!calls.files.contains(&"/proc/hardware".to_string()));
    assert!(!calls.files.contains(&"/proc/asound/cards".to_string()));
    assert!(!calls.files.contains(&"/proc/bus/input/devices".to_string()));
    assert!(!calls
        .file_bytes
        .iter()
        .any(|path| path.contains("/sys/class/drm/")));
    assert!(!calls
        .canonical_paths
        .iter()
        .any(|path| path.contains("/sys/class/video4linux/")));
    assert!(!calls.globs.contains(&"/sys/class/drm/*/edid".to_string()));
}

#[derive(Debug, Default, Clone)]
struct RecordedCalls {
    commands: Vec<String>,
    files: Vec<String>,
    file_bytes: Vec<String>,
    canonical_paths: Vec<String>,
    globs: Vec<String>,
}

#[derive(Debug, Default, Clone)]
struct StrictRecordingRunner {
    calls: Arc<Mutex<RecordedCalls>>,
}

impl StrictRecordingRunner {
    fn new() -> Self {
        Self::default()
    }

    fn recorded_calls(&self) -> RecordedCalls {
        self.calls.lock().unwrap().clone()
    }
}

impl SourceRunner for StrictRecordingRunner {
    fn run_command<'life0, 'life1, 'async_trait>(
        &'life0 self,
        command: &'life1 CommandSpec,
        _timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = SourceResult> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        let source = command.display_name();
        self.calls.lock().unwrap().commands.push(source.clone());
        Box::pin(async move {
            SourceResult::error(
                source,
                SourceErrorKind::Missing,
                "strict command not registered",
            )
        })
    }

    fn read_file<'life0, 'life1, 'async_trait>(
        &'life0 self,
        path: &'life1 Path,
    ) -> Pin<Box<dyn Future<Output = SourceResult> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        let source = path.display().to_string();
        self.calls.lock().unwrap().files.push(source.clone());
        Box::pin(async move {
            SourceResult::error(
                source,
                SourceErrorKind::Missing,
                "strict file not registered",
            )
        })
    }

    fn read_file_bytes<'life0, 'life1, 'async_trait>(
        &'life0 self,
        path: &'life1 Path,
    ) -> Pin<Box<dyn Future<Output = SourceBytesResult> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        let source = path.display().to_string();
        self.calls.lock().unwrap().file_bytes.push(source.clone());
        Box::pin(async move {
            SourceBytesResult::error(
                source,
                SourceErrorKind::Missing,
                "strict bytes file not registered",
            )
        })
    }

    fn canonicalize_path<'life0, 'life1, 'async_trait>(
        &'life0 self,
        path: &'life1 Path,
    ) -> Pin<Box<dyn Future<Output = SourceResult> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        let source = path.display().to_string();
        self.calls
            .lock()
            .unwrap()
            .canonical_paths
            .push(source.clone());
        Box::pin(async move {
            SourceResult::error(
                source,
                SourceErrorKind::Missing,
                "strict canonical path not registered",
            )
        })
    }

    fn glob<'life0, 'life1, 'async_trait>(
        &'life0 self,
        pattern: &'life1 str,
    ) -> Pin<Box<dyn Future<Output = GlobResult> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        let pattern = pattern.to_string();
        self.calls.lock().unwrap().globs.push(pattern.clone());
        Box::pin(async move {
            GlobResult {
                pattern,
                paths: Vec::<PathBuf>::new(),
            }
        })
    }
}
