use clap::Parser;
use hw_cli::args::{Cli, Command, OutputFormat};
use hw_model::DeviceKind;

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
