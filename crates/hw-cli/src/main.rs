use anyhow::Result;
use clap::Parser;
use hw_cli::args::{Cli, Command, ListFormat, OutputFormat};
use hw_cli::exit::{classify_parse_error, exit_code_for_inventory, exit_code_for_status, ExitCode};
use hw_cli::permission::{command_requires_hardware_access, ensure_root};
use hw_inventory::{observe_inventory, InventoryStore, ObserveInventoryOptions};
use hw_model::{ScanConfig, ScanReport};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let code = classify_parse_error(&err).code();
            err.print()?;
            std::process::exit(code);
        }
    };

    if command_requires_hardware_access(&cli.command) {
        if let Err(err) = ensure_root() {
            eprintln!("{err}");
            std::process::exit(ExitCode::Permission.code());
        }
    }

    match cli.command {
        Command::Scan(args) => {
            // Iron rule: inventory always collects and publishes a complete snapshot.
            // --kind / --exclude-kind / --no-* only filter stdout after observation.
            let report = observe_or_exit(
                &args.state_dir,
                ObserveInventoryOptions {
                    force_full_scan: args.force,
                    scan_config: hw_cli::view::inventory_scan_config(args.timeout),
                    ..ObserveInventoryOptions::default()
                },
            )
            .await;
            let report = hw_cli::view::filtered_report(
                report,
                &args.kind,
                &args.exclude_kind,
                args.no_optional_sources,
                args.no_sources,
                args.no_warnings,
            );
            match args.format {
                OutputFormat::Json => {
                    let flat = hw_output::to_flat_report(&report);
                    if args.pretty {
                        println!("{}", serde_json::to_string_pretty(&flat)?);
                    } else {
                        println!("{}", serde_json::to_string(&flat)?);
                    }
                }
                OutputFormat::Jsonl => println!("{}", hw_output::to_jsonl(&report)?),
                OutputFormat::TypedJson => {
                    if args.pretty {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    } else {
                        println!("{}", serde_json::to_string(&report)?);
                    }
                }
                OutputFormat::SummaryJson => {
                    let flat = hw_output::to_flat_report(&report);
                    println!("{}", serde_json::to_string(&flat.summary)?);
                }
            }
            let code = exit_code_for_status(report.status);
            if code != ExitCode::Ok {
                std::process::exit(code.code());
            }
        }
        Command::Summary(args) => {
            let report = observe_or_exit(&args.state_dir, ObserveInventoryOptions::default()).await;
            print!("{}", hw_output::summary_text(&report));
        }
        Command::Table(args) => {
            let report = observe_or_exit(&args.state_dir, ObserveInventoryOptions::default()).await;
            print!("{}", hw_output::table_text(&report, args.kind));
        }
        Command::BindId(args) => {
            let report = observe_or_exit(
                &args.state_dir,
                ObserveInventoryOptions {
                    scan_config: ScanConfig {
                        timeout: args.timeout,
                        ..ScanConfig::default()
                    },
                    ..ObserveInventoryOptions::default()
                },
            )
            .await;
            let report = hw_bindid::BindIdReport::from_scan_report(&report);
            if args.pretty {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", serde_json::to_string(&report)?);
            }
            if report.status == hw_bindid::BindIdStatus::Failed {
                std::process::exit(ExitCode::ScanFailed.code());
            }
        }
        Command::ListKinds { format } => match format {
            ListFormat::Text => println!("{}", hw_output::list_kinds().join("\n")),
            ListFormat::Json => println!("{}", serde_json::to_string(&hw_output::list_kinds())?),
        },
        Command::Schema { version } => {
            if version {
                println!("{}", hw_output::schema_version());
            } else {
                println!("{{\"schema_version\":\"{}\"}}", hw_output::schema_version());
            }
        }
        Command::Sources { format: _ } => {
            println!("{{\"sources\":[]}}");
        }
        Command::Snapshot(args) => {
            if let Err(error) = hw_cli::snapshot::run_snapshot_command(args).await {
                eprintln!("snapshot error [{}]: {error}", error.code());
                std::process::exit(exit_code_for_inventory(&error).code());
            }
        }
    }
    Ok(())
}

async fn observe_or_exit(state_dir: &Path, options: ObserveInventoryOptions) -> ScanReport {
    let result = async {
        let store = InventoryStore::open(state_dir).await?;
        observe_inventory(&store, options).await
    }
    .await;
    match result {
        Ok(observation) => observation.report,
        Err(error) => {
            eprintln!("inventory error [{}]: {error}", error.code());
            std::process::exit(exit_code_for_inventory(&error).code());
        }
    }
}
