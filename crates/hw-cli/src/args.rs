use clap::{Args, Parser, Subcommand, ValueEnum};
use hw_model::DeviceKind;
use std::{str::FromStr, time::Duration};

#[derive(Debug, Parser)]
#[command(
    name = "qurbrix-hw",
    version,
    about = "General-purpose Linux hardware scanner"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Scan(ScanArgs),
    Summary,
    Table(TableArgs),
    #[command(name = "bindid")]
    BindId(BindIdArgs),
    ListKinds {
        #[arg(long, value_enum, default_value_t = ListFormat::Text)]
        format: ListFormat,
    },
    Schema {
        #[arg(long)]
        version: bool,
    },
    Sources {
        #[arg(long, value_enum, default_value_t = SourcesFormat::Json)]
        format: SourcesFormat,
    },
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,
    #[arg(long)]
    pub pretty: bool,
    #[arg(long, value_parser = parse_kind)]
    pub kind: Vec<DeviceKind>,
    #[arg(long, value_parser = parse_kind)]
    pub exclude_kind: Vec<DeviceKind>,
    #[arg(long, default_value = "30s", value_parser = parse_duration)]
    pub timeout: Duration,
    #[arg(long)]
    pub no_optional_sources: bool,
    #[arg(long)]
    pub no_sources: bool,
    #[arg(long)]
    pub no_warnings: bool,
}

#[derive(Debug, Args)]
pub struct TableArgs {
    #[arg(long, value_parser = parse_kind)]
    pub kind: Option<DeviceKind>,
}

#[derive(Debug, Args)]
pub struct BindIdArgs {
    #[arg(long)]
    pub pretty: bool,
    #[arg(long, default_value = "30s", value_parser = parse_duration)]
    pub timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Json,
    Jsonl,
    TypedJson,
    SummaryJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SourcesFormat {
    Json,
}

pub fn parse_kind(value: &str) -> Result<DeviceKind, String> {
    DeviceKind::from_str(value)
}

pub fn parse_duration(value: &str) -> Result<Duration, String> {
    let value = value.trim();
    if let Some(seconds) = value.strip_suffix('s') {
        return seconds
            .parse::<u64>()
            .map(Duration::from_secs)
            .map_err(|err| err.to_string());
    }
    value
        .parse::<u64>()
        .map(Duration::from_secs)
        .map_err(|err| err.to_string())
}
