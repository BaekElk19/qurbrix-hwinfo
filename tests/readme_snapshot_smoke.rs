use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_qurbrix-hw"))
}

#[test]
fn documented_snapshot_help_and_temp_state_commands_run() {
    let english = include_str!("../README.md");
    let chinese = include_str!("../README.zh-CN.md");
    for command in [
        "snapshot ensure",
        "snapshot show",
        "snapshot list",
        "snapshot diff",
        "snapshot export",
    ] {
        assert!(english.contains(command), "English README misses {command}");
        assert!(chinese.contains(command), "Chinese README misses {command}");
    }

    let help = binary().args(["snapshot", "--help"]).output().unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    for command in ["ensure", "show", "list", "diff", "export"] {
        assert!(help.contains(command));
    }

    let state = tempfile::tempdir().unwrap();
    let list = binary()
        .args([
            "snapshot",
            "list",
            "--state-dir",
            state.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(list.status.success());
    assert!(list.stderr.is_empty());
    let output: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(output["schema_version"], "qurbrix.hw.snapshot.cli.v1");
    assert_eq!(output["snapshots"].as_array().unwrap().len(), 0);
}
