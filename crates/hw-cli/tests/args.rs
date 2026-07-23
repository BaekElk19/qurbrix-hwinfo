use clap::Parser;
use hw_cli::args::{BindIdArgs, Cli, Command, OutputFormat, ScanArgs, TableArgs};
use hw_cli::exit::{classify_parse_error, exit_code_for_status, ExitCode};
use hw_cli::permission::{command_requires_hardware_access, ensure_root_with};
use hw_model::DeviceKind;
use hw_model::ScanStatus;
use std::time::Duration;

#[test]
fn parses_scan_json_kind_filter() {
    let cli = Cli::parse_from([
        "qurbrix-hw",
        "scan",
        "--format",
        "json",
        "--kind",
        "storage",
    ]);
    match cli.command {
        Command::Scan(scan) => {
            assert_eq!(scan.format, OutputFormat::Json);
            assert_eq!(scan.kind, vec![DeviceKind::Storage]);
        }
        _ => panic!("expected scan"),
    }
}

#[test]
fn parses_list_kinds() {
    let cli = Cli::parse_from(["qurbrix-hw", "list-kinds"]);
    assert!(matches!(cli.command, Command::ListKinds { .. }));
}

#[test]
fn parses_bindid_pretty_flag() {
    let cli = Cli::parse_from(["qurbrix-hw", "bindid", "--pretty"]);
    match cli.command {
        Command::BindId(args) => {
            assert!(args.pretty);
        }
        _ => panic!("expected bindid"),
    }
}

#[test]
fn bindid_defaults_timeout_to_30_seconds() {
    let cli = Cli::parse_from(["qurbrix-hw", "bindid"]);
    match cli.command {
        Command::BindId(args) => {
            assert_eq!(args.timeout, Duration::from_secs(30));
        }
        _ => panic!("expected bindid"),
    }
}

#[test]
fn bindid_parses_custom_timeout() {
    let cli = Cli::parse_from(["qurbrix-hw", "bindid", "--timeout", "5s"]);
    match cli.command {
        Command::BindId(args) => {
            assert_eq!(args.timeout, Duration::from_secs(5));
        }
        _ => panic!("expected bindid"),
    }
}

#[test]
fn sources_rejects_ignored_non_json_formats() {
    assert!(Cli::try_parse_from(["qurbrix-hw", "sources", "--format", "jsonl"]).is_err());
}

#[test]
fn parses_snapshot_commands_and_duration_units() {
    let cli = Cli::try_parse_from([
        "qurbrix-hw",
        "snapshot",
        "ensure",
        "--state-dir",
        "/tmp/state",
        "--max-age",
        "2h",
        "--force",
        "--reject-partial",
    ])
    .unwrap();
    let Command::Snapshot(args) = cli.command else {
        panic!("expected snapshot command");
    };
    let hw_cli::args::SnapshotCommand::Ensure(args) = args.command else {
        panic!("expected ensure command");
    };
    assert_eq!(args.max_age.as_secs(), 7_200);
    assert!(args.force);
    assert!(args.reject_partial);
}

#[test]
fn only_snapshot_ensure_requires_hardware_access() {
    let ensure = Cli::try_parse_from(["qurbrix-hw", "snapshot", "ensure"]).unwrap();
    assert!(command_requires_hardware_access(&ensure.command));
    let id = hw_model::SnapshotId::new_v7().to_string();
    let show = Cli::try_parse_from(["qurbrix-hw", "snapshot", "show", &id]).unwrap();
    assert!(!command_requires_hardware_access(&show.command));
}

#[test]
fn parses_snapshot_prune_retention_defaults_and_overrides() {
    let cli = Cli::try_parse_from([
        "qurbrix-hw",
        "snapshot",
        "prune",
        "--keep-recent",
        "12",
        "--max-age",
        "30d",
        "--dry-run",
    ])
    .unwrap();
    let Command::Snapshot(args) = cli.command else {
        panic!("expected snapshot command");
    };
    let hw_cli::args::SnapshotCommand::Prune(args) = args.command else {
        panic!("expected prune command");
    };
    assert_eq!(args.keep_recent, 12);
    assert_eq!(args.max_age.as_secs(), 30 * 86_400);
    assert!(args.dry_run);
}

#[test]
fn identifies_hardware_access_commands() {
    assert!(command_requires_hardware_access(&Command::Scan(ScanArgs {
        format: OutputFormat::Json,
        pretty: false,
        kind: Vec::new(),
        exclude_kind: Vec::new(),
        timeout: Duration::from_secs(30),
        no_optional_sources: false,
        no_sources: false,
        no_warnings: false,
    })));
    assert!(command_requires_hardware_access(&Command::Summary));
    assert!(command_requires_hardware_access(&Command::Table(
        TableArgs { kind: None }
    )));
    assert!(command_requires_hardware_access(&Command::BindId(
        BindIdArgs {
            pretty: false,
            timeout: Duration::from_secs(30),
        }
    )));
    assert!(!command_requires_hardware_access(&Command::Schema {
        version: false
    }));
    assert!(!command_requires_hardware_access(&Command::ListKinds {
        format: hw_cli::args::ListFormat::Text,
    }));
    assert!(!command_requires_hardware_access(&Command::Sources {
        format: hw_cli::args::SourcesFormat::Json,
    }));
}

#[test]
fn ensure_root_with_accepts_root_uid() {
    assert!(ensure_root_with(|| 0).is_ok());
}

#[test]
fn ensure_root_with_rejects_non_root_uid() {
    assert!(ensure_root_with(|| 1000).is_err());
}

#[test]
fn maps_failed_scan_status_to_contract_exit_code() {
    assert_eq!(exit_code_for_status(ScanStatus::Complete), ExitCode::Ok);
    assert_eq!(exit_code_for_status(ScanStatus::Partial), ExitCode::Ok);
    assert_eq!(
        exit_code_for_status(ScanStatus::Failed),
        ExitCode::ScanFailed
    );
}

#[test]
fn maps_unsupported_kind_parse_error_to_contract_exit_code() {
    let err = Cli::try_parse_from(["qurbrix-hw", "scan", "--kind", "not-a-kind"]).unwrap_err();
    assert_eq!(classify_parse_error(&err), ExitCode::Unsupported);
}
