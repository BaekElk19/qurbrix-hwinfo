use async_trait::async_trait;
use futures::future::join_all;
use hw_source::{
    is_cacheable_command, CachedSourceRunner, CommandSpec, GlobResult, SourceBytesResult,
    SourceResult, SourceRunner,
};
use std::{
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

#[derive(Default)]
struct DelayedRunner {
    command_calls: AtomicUsize,
    active: AtomicUsize,
    peak: AtomicUsize,
}

struct ActiveGuard<'a>(&'a AtomicUsize);

impl Drop for ActiveGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl SourceRunner for DelayedRunner {
    async fn run_command(&self, command: &CommandSpec, _timeout: Duration) -> SourceResult {
        self.command_calls.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        let _guard = ActiveGuard(&self.active);
        tokio::time::sleep(Duration::from_millis(20)).await;
        SourceResult::success(command.display_name(), command.display_name())
    }

    async fn read_file(&self, path: &Path) -> SourceResult {
        SourceResult::success(path.display().to_string(), "file")
    }

    async fn read_file_bytes(&self, path: &Path) -> SourceBytesResult {
        SourceBytesResult::success(path.display().to_string(), b"file".to_vec())
    }

    async fn canonicalize_path(&self, path: &Path) -> SourceResult {
        SourceResult::success(path.display().to_string(), path.display().to_string())
    }

    async fn glob(&self, pattern: &str) -> GlobResult {
        GlobResult {
            pattern: pattern.to_string(),
            paths: Vec::new(),
        }
    }
}

#[tokio::test]
async fn concurrent_equivalent_requests_execute_once() {
    let inner = DelayedRunner::default();
    let cached = CachedSourceRunner::new(&inner, 4, None, true);
    let command = CommandSpec::new("lspci", ["-nn", "-k"]);
    let results =
        join_all((0..10).map(|_| cached.run_command(&command, Duration::from_secs(1)))).await;
    assert!(results.iter().all(SourceResult::is_success));
    assert_eq!(inner.command_calls.load(Ordering::SeqCst), 1);
    let metrics = cached.metrics().await;
    assert_eq!(metrics.requests, 10);
    assert_eq!(metrics.executions, 1);
    assert_eq!(metrics.cache_hits, 9);
}

#[tokio::test]
async fn command_semaphore_never_exceeds_configured_limit() {
    let inner = DelayedRunner::default();
    let cached = CachedSourceRunner::new(&inner, 3, None, false);
    let commands = (0..12)
        .map(|index| CommandSpec::new("lspci", [format!("-{index}")]))
        .collect::<Vec<_>>();
    join_all(
        commands
            .iter()
            .map(|command| cached.run_command(command, Duration::from_secs(1))),
    )
    .await;
    assert_eq!(inner.peak.load(Ordering::SeqCst), 3);
    assert_eq!(cached.metrics().await.peak_external_commands, 3);
}

#[tokio::test]
async fn commands_outside_read_only_allowlist_are_never_cached() {
    assert!(is_cacheable_command("lspci"));
    assert!(!is_cacheable_command("side-effect-tool"));
    let inner = DelayedRunner::default();
    let cached = CachedSourceRunner::new(&inner, 2, None, true);
    let command = CommandSpec::new("side-effect-tool", ["run"]);
    cached.run_command(&command, Duration::from_secs(1)).await;
    cached.run_command(&command, Duration::from_secs(1)).await;
    assert_eq!(inner.command_calls.load(Ordering::SeqCst), 2);
}
