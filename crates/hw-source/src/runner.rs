use crate::{CommandSpec, GlobResult, SourceBytesResult, SourceErrorKind, SourceResult};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{fs, process::Command, time};

#[async_trait]
pub trait SourceRunner: Send + Sync {
    async fn run_command(&self, command: &CommandSpec, timeout: Duration) -> SourceResult;
    async fn read_file(&self, path: &Path) -> SourceResult;
    async fn read_file_bytes(&self, path: &Path) -> SourceBytesResult;
    async fn glob(&self, pattern: &str) -> GlobResult;
}

#[derive(Debug, Default)]
pub struct RealSourceRunner;

#[async_trait]
impl SourceRunner for RealSourceRunner {
    async fn run_command(&self, command: &CommandSpec, timeout: Duration) -> SourceResult {
        let display = command.display_name();
        let mut cmd = Command::new(&command.program);
        cmd.args(&command.args);
        match time::timeout(timeout, cmd.output()).await {
            Ok(Ok(output)) => SourceResult {
                source: display,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                exit_status: output.status.code(),
                error_kind: if output.status.success() {
                    None
                } else {
                    Some(SourceErrorKind::Failed)
                },
            },
            Ok(Err(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                SourceResult::error(display, SourceErrorKind::Missing, err.to_string())
            }
            Ok(Err(err)) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                SourceResult::error(display, SourceErrorKind::PermissionDenied, err.to_string())
            }
            Ok(Err(err)) => SourceResult::error(display, SourceErrorKind::Failed, err.to_string()),
            Err(_) => SourceResult::error(display, SourceErrorKind::Timeout, "command timed out"),
        }
    }

    async fn read_file(&self, path: &Path) -> SourceResult {
        let source = path.display().to_string();
        match fs::read(path).await {
            Ok(bytes) => SourceResult::success(source, String::from_utf8_lossy(&bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                SourceResult::error(source, SourceErrorKind::Missing, err.to_string())
            }
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                SourceResult::error(source, SourceErrorKind::PermissionDenied, err.to_string())
            }
            Err(err) => SourceResult::error(source, SourceErrorKind::Failed, err.to_string()),
        }
    }

    async fn read_file_bytes(&self, path: &Path) -> SourceBytesResult {
        let source = path.display().to_string();
        match fs::read(path).await {
            Ok(bytes) => SourceBytesResult::success(source, bytes),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                SourceBytesResult::error(source, SourceErrorKind::Missing, err.to_string())
            }
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                SourceBytesResult::error(source, SourceErrorKind::PermissionDenied, err.to_string())
            }
            Err(err) => SourceBytesResult::error(source, SourceErrorKind::Failed, err.to_string()),
        }
    }

    async fn glob(&self, pattern: &str) -> GlobResult {
        let mut paths = Vec::new();

        if let Some((before, after)) = pattern.split_once('*') {
            if before.ends_with('/') && after.starts_with('/') {
                let dir = Path::new(before.trim_end_matches('/'));
                let suffix = after.trim_start_matches('/');
                if let Ok(mut entries) = fs::read_dir(dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let candidate = entry.path().join(suffix);
                        if candidate.exists() {
                            paths.push(candidate);
                        }
                    }
                }
            } else {
                let prefix = pattern.trim_end_matches('*');
                let dir = Path::new(prefix)
                    .parent()
                    .unwrap_or_else(|| Path::new(prefix));
                let name_prefix = Path::new(prefix)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                if let Ok(mut entries) = fs::read_dir(dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if entry
                            .file_name()
                            .to_str()
                            .is_some_and(|name| name.starts_with(name_prefix))
                        {
                            paths.push(entry.path());
                        }
                    }
                }
            }
        } else if Path::new(pattern).exists() {
            paths.push(PathBuf::from(pattern));
        }
        GlobResult {
            pattern: pattern.to_string(),
            paths,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct FakeSourceRunner {
    commands: HashMap<CommandSpec, SourceResult>,
    files: HashMap<PathBuf, SourceResult>,
    file_bytes: HashMap<PathBuf, SourceBytesResult>,
    globs: HashMap<String, Vec<PathBuf>>,
}

impl FakeSourceRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_command(
        mut self,
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
        stdout: impl Into<String>,
    ) -> Self {
        let spec = CommandSpec::new(program, args);
        self.commands.insert(
            spec.clone(),
            SourceResult::success(spec.display_name(), stdout),
        );
        self
    }

    pub fn with_file(mut self, path: impl Into<PathBuf>, contents: impl Into<String>) -> Self {
        let path = path.into();
        self.files.insert(
            path.clone(),
            SourceResult::success(path.display().to_string(), contents),
        );
        self
    }

    pub fn with_file_bytes(mut self, path: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) -> Self {
        let path = path.into();
        self.file_bytes.insert(
            path.clone(),
            SourceBytesResult::success(path.display().to_string(), bytes),
        );
        self
    }

    pub fn with_glob(mut self, pattern: impl Into<String>, paths: Vec<PathBuf>) -> Self {
        self.globs.insert(pattern.into(), paths);
        self
    }
}

#[async_trait]
impl SourceRunner for FakeSourceRunner {
    async fn run_command(&self, command: &CommandSpec, _timeout: Duration) -> SourceResult {
        self.commands.get(command).cloned().unwrap_or_else(|| {
            SourceResult::error(
                command.display_name(),
                SourceErrorKind::Missing,
                "fake command not registered",
            )
        })
    }

    async fn read_file(&self, path: &Path) -> SourceResult {
        self.files.get(path).cloned().unwrap_or_else(|| {
            SourceResult::error(
                path.display().to_string(),
                SourceErrorKind::Missing,
                "fake file not registered",
            )
        })
    }

    async fn read_file_bytes(&self, path: &Path) -> SourceBytesResult {
        self.file_bytes.get(path).cloned().unwrap_or_else(|| {
            SourceBytesResult::error(
                path.display().to_string(),
                SourceErrorKind::Missing,
                "fake file not registered",
            )
        })
    }

    async fn glob(&self, pattern: &str) -> GlobResult {
        GlobResult {
            pattern: pattern.to_string(),
            paths: self.globs.get(pattern).cloned().unwrap_or_default(),
        }
    }
}
