use hw_model::{Device, DeviceRef, ScanWarning};
use hw_source::{SourceErrorKind, SourceResult};

#[derive(Debug, Clone, Default)]
pub struct ProbeResult {
    pub devices: Vec<Device>,
    pub warnings: Vec<ScanWarning>,
    pub consumed: Vec<DeviceRef>,
}

impl ProbeResult {
    pub fn with_devices(devices: Vec<Device>) -> Self {
        Self {
            devices,
            warnings: Vec::new(),
            consumed: Vec::new(),
        }
    }

    pub fn source_failure(probe: &str, result: &SourceResult) -> Self {
        let kind = result.error_kind.unwrap_or(SourceErrorKind::Failed);
        let code = match kind {
            SourceErrorKind::Missing => "source_missing",
            SourceErrorKind::PermissionDenied => "source_permission_denied",
            SourceErrorKind::Timeout => "source_timeout",
            SourceErrorKind::Failed => "source_failed",
        };
        let detail = result.stderr.trim();
        let message = if detail.is_empty() {
            format!("{probe} source '{}' failed: {kind:?}", result.source)
        } else {
            format!("{probe} source '{}' failed: {detail}", result.source)
        };
        Self {
            devices: Vec::new(),
            warnings: vec![ScanWarning::new(code, message).with_source(result.source.clone())],
            consumed: Vec::new(),
        }
    }
}
