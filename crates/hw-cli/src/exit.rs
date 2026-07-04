#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Ok = 0,
    CliOrSerialization = 1,
    ScanFailed = 2,
    Unsupported = 3,
    Permission = 4,
    Timeout = 124,
}

impl ExitCode {
    pub fn code(self) -> i32 {
        self as i32
    }
}
