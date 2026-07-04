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
