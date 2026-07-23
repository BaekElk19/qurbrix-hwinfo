use crate::{
    CommandSpec, GlobResult, SourceBytesResult, SourceErrorKind, SourceResult, SourceRunner,
};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{Mutex, OnceCell, Semaphore};
use tokio::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceObservation {
    pub source: String,
    pub duration_micros: u64,
    pub cache_hit: bool,
    pub timed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMetrics {
    pub requests: u64,
    pub executions: u64,
    pub cache_hits: u64,
    pub commands_started: u64,
    pub peak_external_commands: usize,
    pub observations: Vec<SourceObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CommandCacheKey {
    command: CommandSpec,
    timeout_millis: u64,
}

pub struct CachedSourceRunner<'a> {
    inner: &'a dyn SourceRunner,
    command_limit: Arc<Semaphore>,
    deadline: Option<Instant>,
    cache_enabled: bool,
    commands: Mutex<HashMap<CommandCacheKey, Arc<OnceCell<SourceResult>>>>,
    files: Mutex<HashMap<PathBuf, Arc<OnceCell<SourceResult>>>>,
    file_bytes: Mutex<HashMap<PathBuf, Arc<OnceCell<SourceBytesResult>>>>,
    canonical_paths: Mutex<HashMap<PathBuf, Arc<OnceCell<SourceResult>>>>,
    globs: Mutex<HashMap<String, Arc<OnceCell<GlobResult>>>>,
    requests: AtomicU64,
    executions: AtomicU64,
    commands_started: AtomicU64,
    active_commands: AtomicUsize,
    peak_commands: AtomicUsize,
    observations: Mutex<Vec<SourceObservation>>,
}

struct ActiveCommandGuard<'a> {
    active: &'a AtomicUsize,
}

impl Drop for ActiveCommandGuard<'_> {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

impl<'a> CachedSourceRunner<'a> {
    pub fn new(
        inner: &'a dyn SourceRunner,
        max_external_commands: usize,
        deadline: Option<Instant>,
        cache_enabled: bool,
    ) -> Self {
        Self {
            inner,
            command_limit: Arc::new(Semaphore::new(max_external_commands.max(1))),
            deadline,
            cache_enabled,
            commands: Mutex::new(HashMap::new()),
            files: Mutex::new(HashMap::new()),
            file_bytes: Mutex::new(HashMap::new()),
            canonical_paths: Mutex::new(HashMap::new()),
            globs: Mutex::new(HashMap::new()),
            requests: AtomicU64::new(0),
            executions: AtomicU64::new(0),
            commands_started: AtomicU64::new(0),
            active_commands: AtomicUsize::new(0),
            peak_commands: AtomicUsize::new(0),
            observations: Mutex::new(Vec::new()),
        }
    }

    pub async fn metrics(&self) -> SourceMetrics {
        let requests = self.requests.load(Ordering::Relaxed);
        let executions = self.executions.load(Ordering::Relaxed);
        SourceMetrics {
            requests,
            executions,
            cache_hits: requests.saturating_sub(executions),
            commands_started: self.commands_started.load(Ordering::Relaxed),
            peak_external_commands: self.peak_commands.load(Ordering::Relaxed),
            observations: self.observations.lock().await.clone(),
        }
    }

    fn remaining(&self) -> Option<Duration> {
        self.deadline
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    fn effective_timeout(&self, requested: Duration) -> Duration {
        self.remaining()
            .map(|remaining| requested.min(remaining))
            .unwrap_or(requested)
    }

    async fn observe(&self, source: String, started: Instant, cache_hit: bool, timed_out: bool) {
        self.observations.lock().await.push(SourceObservation {
            source,
            duration_micros: started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
            cache_hit,
            timed_out,
        });
    }

    async fn execute_command(&self, command: &CommandSpec, timeout: Duration) -> SourceResult {
        let timeout = self.effective_timeout(timeout);
        if timeout.is_zero() {
            return SourceResult::error(
                command.display_name(),
                SourceErrorKind::Timeout,
                "global scan deadline exceeded",
            );
        }
        let permit = match self.command_limit.acquire().await {
            Ok(permit) => permit,
            Err(_) => {
                return SourceResult::error(
                    command.display_name(),
                    SourceErrorKind::Failed,
                    "command semaphore closed",
                )
            }
        };
        let active = self.active_commands.fetch_add(1, Ordering::SeqCst) + 1;
        let active_guard = ActiveCommandGuard {
            active: &self.active_commands,
        };
        self.peak_commands.fetch_max(active, Ordering::SeqCst);
        self.commands_started.fetch_add(1, Ordering::Relaxed);
        self.executions.fetch_add(1, Ordering::Relaxed);
        let result = self.inner.run_command(command, timeout).await;
        drop(active_guard);
        drop(permit);
        result
    }

    async fn execute_file(&self, path: &Path) -> SourceResult {
        self.executions.fetch_add(1, Ordering::Relaxed);
        match self.remaining() {
            Some(remaining) if remaining.is_zero() => SourceResult::error(
                path.display().to_string(),
                SourceErrorKind::Timeout,
                "global scan deadline exceeded",
            ),
            Some(remaining) => {
                match tokio::time::timeout(remaining, self.inner.read_file(path)).await {
                    Ok(result) => result,
                    Err(_) => SourceResult::error(
                        path.display().to_string(),
                        SourceErrorKind::Timeout,
                        "global scan deadline exceeded",
                    ),
                }
            }
            None => self.inner.read_file(path).await,
        }
    }

    async fn execute_file_bytes(&self, path: &Path) -> SourceBytesResult {
        self.executions.fetch_add(1, Ordering::Relaxed);
        match self.remaining() {
            Some(remaining) if remaining.is_zero() => SourceBytesResult::error(
                path.display().to_string(),
                SourceErrorKind::Timeout,
                "global scan deadline exceeded",
            ),
            Some(remaining) => {
                match tokio::time::timeout(remaining, self.inner.read_file_bytes(path)).await {
                    Ok(result) => result,
                    Err(_) => SourceBytesResult::error(
                        path.display().to_string(),
                        SourceErrorKind::Timeout,
                        "global scan deadline exceeded",
                    ),
                }
            }
            None => self.inner.read_file_bytes(path).await,
        }
    }

    async fn execute_canonicalize(&self, path: &Path) -> SourceResult {
        self.executions.fetch_add(1, Ordering::Relaxed);
        match self.remaining() {
            Some(remaining) if remaining.is_zero() => SourceResult::error(
                path.display().to_string(),
                SourceErrorKind::Timeout,
                "global scan deadline exceeded",
            ),
            Some(remaining) => {
                match tokio::time::timeout(remaining, self.inner.canonicalize_path(path)).await {
                    Ok(result) => result,
                    Err(_) => SourceResult::error(
                        path.display().to_string(),
                        SourceErrorKind::Timeout,
                        "global scan deadline exceeded",
                    ),
                }
            }
            None => self.inner.canonicalize_path(path).await,
        }
    }

    async fn execute_glob(&self, pattern: &str) -> GlobResult {
        self.executions.fetch_add(1, Ordering::Relaxed);
        match self.remaining() {
            Some(remaining) if remaining.is_zero() => GlobResult {
                pattern: pattern.to_string(),
                paths: Vec::new(),
            },
            Some(remaining) => {
                match tokio::time::timeout(remaining, self.inner.glob(pattern)).await {
                    Ok(result) => result,
                    Err(_) => GlobResult {
                        pattern: pattern.to_string(),
                        paths: Vec::new(),
                    },
                }
            }
            None => self.inner.glob(pattern).await,
        }
    }
}

#[async_trait]
impl SourceRunner for CachedSourceRunner<'_> {
    async fn run_command(&self, command: &CommandSpec, timeout: Duration) -> SourceResult {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        let key = CommandCacheKey {
            command: command.clone(),
            timeout_millis: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        };
        let cacheable = self.cache_enabled && is_cacheable_command(&command.program);
        let (result, cache_hit) = if cacheable {
            let cell = {
                let mut commands = self.commands.lock().await;
                commands
                    .entry(key)
                    .or_insert_with(|| Arc::new(OnceCell::new()))
                    .clone()
            };
            let cache_hit = cell.get().is_some();
            let result = cell
                .get_or_init(|| self.execute_command(command, timeout))
                .await
                .clone();
            (result, cache_hit)
        } else {
            (self.execute_command(command, timeout).await, false)
        };
        self.observe(
            command.display_name(),
            started,
            cache_hit,
            result.error_kind == Some(SourceErrorKind::Timeout),
        )
        .await;
        result
    }

    async fn read_file(&self, path: &Path) -> SourceResult {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        let cell = {
            let mut files = self.files.lock().await;
            files
                .entry(path.to_path_buf())
                .or_insert_with(|| Arc::new(OnceCell::new()))
                .clone()
        };
        let cache_hit = cell.get().is_some();
        let result = if self.cache_enabled {
            cell.get_or_init(|| self.execute_file(path)).await.clone()
        } else {
            self.execute_file(path).await
        };
        self.observe(
            path.display().to_string(),
            started,
            cache_hit && self.cache_enabled,
            result.error_kind == Some(SourceErrorKind::Timeout),
        )
        .await;
        result
    }

    async fn read_file_bytes(&self, path: &Path) -> SourceBytesResult {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        let cell = {
            let mut files = self.file_bytes.lock().await;
            files
                .entry(path.to_path_buf())
                .or_insert_with(|| Arc::new(OnceCell::new()))
                .clone()
        };
        let cache_hit = cell.get().is_some();
        let result = if self.cache_enabled {
            cell.get_or_init(|| self.execute_file_bytes(path))
                .await
                .clone()
        } else {
            self.execute_file_bytes(path).await
        };
        self.observe(
            path.display().to_string(),
            started,
            cache_hit && self.cache_enabled,
            result.error_kind == Some(SourceErrorKind::Timeout),
        )
        .await;
        result
    }

    async fn canonicalize_path(&self, path: &Path) -> SourceResult {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        let cell = {
            let mut paths = self.canonical_paths.lock().await;
            paths
                .entry(path.to_path_buf())
                .or_insert_with(|| Arc::new(OnceCell::new()))
                .clone()
        };
        let cache_hit = cell.get().is_some();
        let result = if self.cache_enabled {
            cell.get_or_init(|| self.execute_canonicalize(path))
                .await
                .clone()
        } else {
            self.execute_canonicalize(path).await
        };
        self.observe(
            path.display().to_string(),
            started,
            cache_hit && self.cache_enabled,
            result.error_kind == Some(SourceErrorKind::Timeout),
        )
        .await;
        result
    }

    async fn glob(&self, pattern: &str) -> GlobResult {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        let cell = {
            let mut globs = self.globs.lock().await;
            globs
                .entry(pattern.to_string())
                .or_insert_with(|| Arc::new(OnceCell::new()))
                .clone()
        };
        let cache_hit = cell.get().is_some();
        let result = if self.cache_enabled {
            cell.get_or_init(|| self.execute_glob(pattern))
                .await
                .clone()
        } else {
            self.execute_glob(pattern).await
        };
        self.observe(
            pattern.to_string(),
            started,
            cache_hit && self.cache_enabled,
            false,
        )
        .await;
        result
    }
}

pub fn is_cacheable_command(program: &str) -> bool {
    matches!(
        Path::new(program)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(program),
        "cat"
            | "dmidecode"
            | "ethtool"
            | "glxinfo"
            | "hciconfig"
            | "hdparm"
            | "hwinfo"
            | "ip"
            | "lpstat"
            | "lscpu"
            | "lshw"
            | "lsblk"
            | "lspci"
            | "lsusb"
            | "modinfo"
            | "nvidia-settings"
            | "nvidia-smi"
            | "pactl"
            | "smartctl"
            | "spd-decode"
            | "upower"
            | "v4l2-ctl"
            | "xrandr"
    )
}
