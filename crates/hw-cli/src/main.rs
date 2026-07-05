use anyhow::Result;
use clap::Parser;
use hw_cli::args::{Cli, Command, ListFormat, OutputFormat};
use hw_cli::exit::{classify_parse_error, exit_code_for_status, ExitCode};
use hw_model::ScanConfig;

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
    match cli.command {
        Command::Scan(args) => {
            let config = ScanConfig {
                kinds: if args.kind.is_empty() {
                    None
                } else {
                    Some(args.kind)
                },
                exclude_kinds: args.exclude_kind,
                timeout: args.timeout,
                optional_sources: !args.no_optional_sources,
                include_sources: !args.no_sources,
                include_warnings: !args.no_warnings,
            };
            let report = hw_collect::collect_scan_report(config).await?;
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
        Command::Summary => {
            let report = hw_collect::collect_scan_report(ScanConfig::default()).await?;
            print!("{}", hw_output::summary_text(&report));
        }
        Command::Table(args) => {
            let report = hw_collect::collect_scan_report(ScanConfig::default()).await?;
            print!("{}", hw_output::table_text(&report, args.kind));
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
    }
    Ok(())
}
