#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceErrorKind {
    Missing,
    PermissionDenied,
    Timeout,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceResult {
    pub source: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_status: Option<i32>,
    pub error_kind: Option<SourceErrorKind>,
}

impl SourceResult {
    pub fn success(source: impl Into<String>, stdout: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            stdout: stdout.into(),
            stderr: String::new(),
            exit_status: Some(0),
            error_kind: None,
        }
    }

    pub fn error(
        source: impl Into<String>,
        kind: SourceErrorKind,
        stderr: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            stdout: String::new(),
            stderr: stderr.into(),
            exit_status: None,
            error_kind: Some(kind),
        }
    }

    pub fn is_success(&self) -> bool {
        self.error_kind.is_none() && self.exit_status == Some(0)
    }
}
