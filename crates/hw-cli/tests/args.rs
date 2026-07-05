use clap::Parser;
use hw_cli::args::{Cli, Command, OutputFormat};
use hw_cli::exit::{classify_parse_error, exit_code_for_status, ExitCode};
use hw_model::DeviceKind;
use hw_model::ScanStatus;

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
