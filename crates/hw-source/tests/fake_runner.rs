use hw_source::{CommandSpec, FakeSourceRunner, SourceErrorKind, SourceRunner};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[tokio::test]
async fn fake_runner_returns_registered_command_output() {
    let runner = FakeSourceRunner::new().with_command(
        "lspci",
        ["-nn", "-k"],
        "00:1f.3 Audio device [0403]: Intel [8086:a348]\n",
    );

    let result = runner
        .run_command(
            &CommandSpec::new("lspci", ["-nn", "-k"]),
            Duration::from_secs(1),
        )
        .await;

    assert!(result.is_success());
    assert!(result.stdout.contains("Audio device"));
}

#[tokio::test]
async fn fake_runner_returns_registered_command_with_status() {
    let runner = FakeSourceRunner::new().with_command_status(
        "smartctl",
        ["-a", "-j", "/dev/sda"],
        r#"{"smart_status":{"passed":false}}"#,
        8,
    );

    let result = runner
        .run_command(
            &CommandSpec::new("smartctl", ["-a", "-j", "/dev/sda"]),
            Duration::from_secs(1),
        )
        .await;

    assert_eq!(result.exit_status, Some(8));
    assert_eq!(result.error_kind, Some(SourceErrorKind::Failed));
    assert!(result.stdout.contains("smart_status"));
}

#[tokio::test]
async fn fake_runner_reports_missing_file() {
    let runner = FakeSourceRunner::new();
    let result = runner.read_file(Path::new("/sys/missing")).await;
    assert_eq!(result.error_kind, Some(SourceErrorKind::Missing));
}

#[tokio::test]
async fn fake_runner_returns_registered_binary_file() {
    let runner = FakeSourceRunner::new()
        .with_file_bytes("/sys/class/drm/card0-HDMI-A-1/edid", vec![0, 255, 1]);

    let result = runner
        .read_file_bytes(Path::new("/sys/class/drm/card0-HDMI-A-1/edid"))
        .await;

    assert!(result.is_success());
    assert_eq!(result.bytes, vec![0, 255, 1]);
}

#[tokio::test]
async fn fake_runner_returns_registered_canonical_path() {
    let runner = FakeSourceRunner::new().with_canonical_path(
        "/sys/block/sda/device",
        "/sys/devices/pci0000:00/0000:00:17.0",
    );

    let result = runner
        .canonicalize_path(Path::new("/sys/block/sda/device"))
        .await;

    assert!(result.is_success());
    assert_eq!(result.stdout, "/sys/devices/pci0000:00/0000:00:17.0");
}

#[tokio::test]
async fn real_runner_reports_missing_command() {
    let runner = hw_source::RealSourceRunner;
    let result = runner
        .run_command(
            &CommandSpec::new("__qurbrix_hw_missing_command__", std::iter::empty::<&str>()),
            Duration::from_secs(1),
        )
        .await;

    assert_eq!(result.error_kind, Some(SourceErrorKind::Missing));
    assert_eq!(result.exit_status, None);
    assert!(result.stderr.contains("No such file") || !result.stderr.is_empty());
}

#[tokio::test]
async fn real_runner_canonicalizes_existing_path() {
    let root = std::env::temp_dir().join(format!("qurbrix-hw-canon-{}", std::process::id()));
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).unwrap();

    let runner = hw_source::RealSourceRunner;
    let result = runner.canonicalize_path(&nested).await;

    std::fs::remove_dir(&nested).unwrap();
    std::fs::remove_dir(&root).unwrap();
    assert!(result.is_success());
    assert!(Path::new(&result.stdout).is_absolute());
    assert!(result.stdout.ends_with("nested"));
}

#[tokio::test]
async fn real_runner_forces_stable_english_command_locale() {
    let runner = hw_source::RealSourceRunner;
    let result = runner
        .run_command(
            &CommandSpec::new("env", std::iter::empty::<&str>()),
            Duration::from_secs(1),
        )
        .await;

    assert!(result.is_success());
    assert!(result.stdout.lines().any(|line| line == "LC_ALL=C"));
    assert!(result.stdout.lines().any(|line| line == "LANG=C"));
    assert!(result.stdout.lines().any(|line| line == "LANGUAGE=en"));
}

#[tokio::test]
async fn real_runner_reads_non_utf8_file_lossily() {
    let path = std::env::temp_dir().join(format!("qurbrix-hw-non-utf8-{}", std::process::id()));
    std::fs::write(&path, [b'o', b'k', 0xff]).unwrap();

    let runner = hw_source::RealSourceRunner;
    let result = runner.read_file(&path).await;

    std::fs::remove_file(&path).unwrap();
    assert!(result.is_success());
    assert_eq!(result.stdout, "ok\u{fffd}");
}

#[tokio::test]
async fn real_runner_reads_binary_file_exactly() {
    let path = std::env::temp_dir().join(format!("qurbrix-hw-binary-{}", std::process::id()));
    std::fs::write(&path, [0x00, 0xff, 0x01]).unwrap();

    let runner = hw_source::RealSourceRunner;
    let result = runner.read_file_bytes(&path).await;

    std::fs::remove_file(&path).unwrap();
    assert!(result.is_success());
    assert_eq!(result.bytes, vec![0x00, 0xff, 0x01]);
}

#[tokio::test]
async fn real_runner_glob_matches_single_middle_wildcard_with_suffix() {
    let root = std::env::temp_dir().join(format!("qurbrix-hw-glob-{}", std::process::id()));
    let connector = root.join("card0-HDMI-A-1");
    let edid = connector.join("edid");
    std::fs::create_dir_all(&connector).unwrap();
    std::fs::write(&edid, [0u8]).unwrap();

    let pattern = format!("{}/*/edid", root.display());
    let runner = hw_source::RealSourceRunner;
    let result = runner.glob(&pattern).await;

    std::fs::remove_file(&edid).unwrap();
    std::fs::remove_dir(&connector).unwrap();
    std::fs::remove_dir(&root).unwrap();
    assert_eq!(result.pattern, pattern);
    assert_paths_contain(&result.paths, &edid);
}

#[tokio::test]
async fn real_runner_glob_matches_children_for_trailing_wildcard() {
    let root =
        std::env::temp_dir().join(format!("qurbrix-hw-glob-trailing-{}", std::process::id()));
    let child = root.join("pci:e1000e");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&child, []).unwrap();

    let pattern = format!("{}/*", root.display());
    let runner = hw_source::RealSourceRunner;
    let result = runner.glob(&pattern).await;

    std::fs::remove_file(&child).unwrap();
    std::fs::remove_dir(&root).unwrap();
    assert_eq!(result.pattern, pattern);
    assert_eq!(result.paths, vec![child]);
}

fn assert_paths_contain(paths: &[PathBuf], expected: &Path) {
    assert!(
        paths.iter().any(|path| path == expected),
        "expected {expected:?} in {paths:?}"
    );
}
