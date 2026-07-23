use clap::error::ErrorKind;
use hw_inventory::InventoryError;
use hw_model::ScanStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Ok = 0,
    CliOrSerialization = 1,
    ScanFailed = 2,
    Unsupported = 3,
    Permission = 4,
    NotFound = 5,
    Storage = 6,
    Timeout = 124,
}

pub fn exit_code_for_inventory(error: &InventoryError) -> ExitCode {
    match error {
        InventoryError::FullScanFailed
        | InventoryError::PartialRejected
        | InventoryError::CoreIdentityIncomplete => ExitCode::ScanFailed,
        InventoryError::LeaseTimeout => ExitCode::Timeout,
        InventoryError::SnapshotNotFound(_) => ExitCode::NotFound,
        InventoryError::Serialization(_) => ExitCode::CliOrSerialization,
        InventoryError::Io(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            ExitCode::Permission
        }
        _ => ExitCode::Storage,
    }
}

impl ExitCode {
    pub fn code(self) -> i32 {
        self as i32
    }
}

pub fn exit_code_for_status(status: ScanStatus) -> ExitCode {
    match status {
        ScanStatus::Complete | ScanStatus::Partial => ExitCode::Ok,
        ScanStatus::Failed => ExitCode::ScanFailed,
    }
}

pub fn classify_parse_error(error: &clap::Error) -> ExitCode {
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => ExitCode::Ok,
        _ if error.to_string().contains("unsupported device kind:") => ExitCode::Unsupported,
        _ => ExitCode::CliOrSerialization,
    }
}
