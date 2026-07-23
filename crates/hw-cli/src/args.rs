use clap::{Args, Parser, Subcommand, ValueEnum};
use hw_model::DeviceKind;
use hw_model::SnapshotId;
use std::{path::PathBuf, str::FromStr, time::Duration};

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
    Snapshot(SnapshotArgs),
}

#[derive(Debug, Args)]
pub struct SnapshotArgs {
    #[command(subcommand)]
    pub command: SnapshotCommand,
}

#[derive(Debug, Subcommand)]
pub enum SnapshotCommand {
    Ensure(SnapshotEnsureArgs),
    Show(SnapshotShowArgs),
    List(SnapshotListArgs),
    Diff(SnapshotDiffArgs),
    Export(SnapshotExportArgs),
    Health(SnapshotHealthArgs),
    Prune(SnapshotPruneArgs),
    Pin(SnapshotPinArgs),
    MarkUploaded(SnapshotMarkUploadedArgs),
}

#[derive(Debug, Args)]
pub struct SnapshotHealthArgs {
    #[arg(long, default_value = "/var/lib/qurbrix-hwinfo")]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotPruneArgs {
    #[arg(long, default_value = "/var/lib/qurbrix-hwinfo")]
    pub state_dir: PathBuf,
    #[arg(long, default_value_t = 30)]
    pub keep_recent: u32,
    #[arg(long, default_value = "90d", value_parser = parse_duration)]
    pub max_age: Duration,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotPinArgs {
    #[arg(value_parser = parse_snapshot_id)]
    pub snapshot_id: SnapshotId,
    #[arg(long, default_value = "/var/lib/qurbrix-hwinfo")]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub unset: bool,
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotMarkUploadedArgs {
    #[arg(value_parser = parse_snapshot_id)]
    pub snapshot_id: SnapshotId,
    #[arg(long, default_value = "/var/lib/qurbrix-hwinfo")]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub at: Option<String>,
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotEnsureArgs {
    #[arg(long, default_value = "/var/lib/qurbrix-hwinfo")]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub force: bool,
    #[arg(long, default_value = "24h", value_parser = parse_duration)]
    pub max_age: Duration,
    #[arg(long)]
    pub reject_partial: bool,
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotShowArgs {
    #[arg(value_parser = parse_snapshot_id)]
    pub snapshot_id: SnapshotId,
    #[arg(long, default_value = "/var/lib/qurbrix-hwinfo")]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotListArgs {
    #[arg(long, default_value = "/var/lib/qurbrix-hwinfo")]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub machine_bind_id: Option<String>,
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=1000))]
    pub limit: u32,
    #[arg(long, default_value_t = 0)]
    pub offset: u64,
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotDiffArgs {
    #[arg(value_parser = parse_snapshot_id)]
    pub from_snapshot_id: SnapshotId,
    #[arg(value_parser = parse_snapshot_id)]
    pub to_snapshot_id: SnapshotId,
    #[arg(long, default_value = "/var/lib/qurbrix-hwinfo")]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotExportArgs {
    #[arg(value_parser = parse_snapshot_id)]
    pub snapshot_id: SnapshotId,
    #[arg(long, default_value = "/var/lib/qurbrix-hwinfo")]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long)]
    pub overwrite: bool,
    #[arg(long)]
    pub pretty: bool,
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
    for (suffix, multiplier) in [('d', 86_400), ('h', 3_600), ('m', 60), ('s', 1)] {
        if let Some(amount) = value.strip_suffix(suffix) {
            let amount = amount.parse::<u64>().map_err(|error| error.to_string())?;
            let seconds = amount
                .checked_mul(multiplier)
                .ok_or_else(|| "duration is too large".to_string())?;
            return Ok(Duration::from_secs(seconds));
        }
    }
    value
        .parse::<u64>()
        .map(Duration::from_secs)
        .map_err(|err| err.to_string())
}

pub fn parse_snapshot_id(value: &str) -> Result<SnapshotId, String> {
    SnapshotId::from_str(value).map_err(|error| error.to_string())
}
