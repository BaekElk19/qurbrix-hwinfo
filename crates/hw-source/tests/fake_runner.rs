use hw_source::{CommandSpec, FakeSourceRunner, SourceErrorKind, SourceRunner};
use std::{path::Path, time::Duration};

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
async fn fake_runner_reports_missing_file() {
    let runner = FakeSourceRunner::new();
    let result = runner.read_file(Path::new("/sys/missing")).await;
    assert_eq!(result.error_kind, Some(SourceErrorKind::Missing));
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
async fn real_runner_reads_non_utf8_file_lossily() {
    let path = std::env::temp_dir().join(format!("qurbrix-hw-non-utf8-{}", std::process::id()));
    std::fs::write(&path, [b'o', b'k', 0xff]).unwrap();

    let runner = hw_source::RealSourceRunner;
    let result = runner.read_file(&path).await;

    std::fs::remove_file(&path).unwrap();
    assert!(result.is_success());
    assert_eq!(result.stdout, "ok\u{fffd}");
}
