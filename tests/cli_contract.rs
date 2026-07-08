use std::process::Command;

fn qurbrix_hw() -> Command {
    Command::new(env!("CARGO_BIN_EXE_qurbrix-hw"))
}

#[test]
fn schema_command_writes_stable_json_to_stdout_only() {
    let output = qurbrix_hw().arg("schema").output().expect("run schema");

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "stderr must be reserved for diagnostics"
    );

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("schema stdout should be JSON");
    assert_eq!(
        value.get("schema_version").and_then(|value| value.as_str()),
        Some("qurbrix.hw.scan.v1")
    );
}

#[test]
fn list_kinds_json_is_machine_readable_stdout_only() {
    let output = qurbrix_hw()
        .args(["list-kinds", "--format", "json"])
        .output()
        .expect("run list-kinds");

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "stderr must be reserved for diagnostics"
    );

    let kinds: Vec<String> =
        serde_json::from_slice(&output.stdout).expect("list-kinds stdout should be JSON");
    assert!(kinds.contains(&"cpu".to_string()));
    assert!(kinds.contains(&"storage".to_string()));
    assert!(kinds.contains(&"other-device".to_string()));
}
