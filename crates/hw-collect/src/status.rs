use hw_model::{ScanStatus, ScanWarning};

pub fn status_from_warnings(warnings: &[ScanWarning], device_count: usize) -> ScanStatus {
    if device_count == 0 {
        ScanStatus::Failed
    } else if warnings.is_empty() {
        ScanStatus::Complete
    } else {
        ScanStatus::Partial
    }
}
