use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, Device, DeviceKind, DeviceProperties, PrinterInfo, ScanWarning, SourceEvidence,
    SourceKind, SourceStatus,
};
use hw_parser::{parse_lpstat_a, parse_lpstat_d, parse_lpstat_v};
use hw_source::CommandSpec;

pub struct PrinterProbe;

#[async_trait]
impl Probe for PrinterProbe {
    fn name(&self) -> &'static str {
        "printer"
    }

    fn kinds(&self) -> &'static [DeviceKind] {
        &[DeviceKind::Printer]
    }

    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult {
        let status = ctx
            .runner
            .run_command(&CommandSpec::new("lpstat", ["-a"]), ctx.timeout)
            .await;
        if !status.is_success() {
            let mut fallback = ProbeResult::source_failure(self.name(), &status);
            let uri_result = ctx
                .runner
                .run_command(&CommandSpec::new("lpstat", ["-v"]), ctx.timeout)
                .await;
            let default = read_default_printer(ctx).await;
            if uri_result.is_success() {
                fallback.devices = devices_from_uris(
                    parse_lpstat_v(&uri_result.stdout),
                    &uri_result.source,
                    default.as_ref(),
                );
            } else {
                fallback
                    .warnings
                    .extend(ProbeResult::source_failure(self.name(), &uri_result).warnings);
            }
            return fallback;
        }
        let uri_result = ctx
            .runner
            .run_command(&CommandSpec::new("lpstat", ["-v"]), ctx.timeout)
            .await;
        let mut warnings = Vec::new();
        let uris = if uri_result.is_success() {
            parse_lpstat_v(&uri_result.stdout)
        } else {
            warnings.extend(ProbeResult::source_failure(self.name(), &uri_result).warnings);
            Vec::new()
        };
        let default = read_default_printer(ctx).await;
        let statuses = parse_lpstat_a(&status.stdout);
        if statuses.is_empty() {
            warnings.push(
                ScanWarning::new("source_empty", "printer source produced no queue records")
                    .with_source(status.source),
            );
            let devices = if uri_result.is_success() {
                devices_from_uris(uris, &uri_result.source, default.as_ref())
            } else {
                Vec::new()
            };
            return ProbeResult {
                devices,
                warnings,
                consumed: Vec::new(),
            };
        }
        let devices = statuses
            .into_iter()
            .map(|printer| {
                let uri = uris
                    .iter()
                    .find(|u| u.queue == printer.queue)
                    .and_then(|u| u.device_uri.clone());
                let is_default = default
                    .as_ref()
                    .map(|default| default.queue == printer.queue);
                let device = Device::new(
                    device_id::printer(&printer.queue),
                    DeviceKind::Printer,
                    printer.queue.clone(),
                    DeviceProperties::Printer(PrinterInfo {
                        queue_name: Some(printer.queue),
                        accepting: Some(printer.accepting),
                        device_uri: uri,
                        make_model: None,
                        is_default,
                    }),
                )
                .with_source(SourceEvidence {
                    source: status.source.clone(),
                    kind: SourceKind::Command,
                    status: SourceStatus::Success,
                    summary: None,
                });
                with_default_source(device, default.as_ref())
            })
            .collect();
        ProbeResult {
            devices,
            warnings,
            consumed: Vec::new(),
        }
    }
}

struct PrinterDefault {
    queue: String,
    source: String,
}

async fn read_default_printer(ctx: &ProbeContext<'_>) -> Option<PrinterDefault> {
    let result = ctx
        .runner
        .run_command(&CommandSpec::new("lpstat", ["-d"]), ctx.timeout)
        .await;
    if !result.is_success() {
        return None;
    }
    parse_lpstat_d(&result.stdout).map(|queue| PrinterDefault {
        queue,
        source: result.source,
    })
}

fn devices_from_uris(
    uris: Vec<hw_parser::PrinterUriRecord>,
    source: &str,
    default: Option<&PrinterDefault>,
) -> Vec<Device> {
    uris.into_iter()
        .map(|printer| {
            let is_default = default.map(|default| default.queue == printer.queue);
            let device = Device::new(
                device_id::printer(&printer.queue),
                DeviceKind::Printer,
                printer.queue.clone(),
                DeviceProperties::Printer(PrinterInfo {
                    queue_name: Some(printer.queue),
                    accepting: None,
                    device_uri: printer.device_uri,
                    make_model: None,
                    is_default,
                }),
            )
            .with_source(SourceEvidence {
                source: source.to_string(),
                kind: SourceKind::Command,
                status: SourceStatus::Success,
                summary: None,
            });
            with_default_source(device, default)
        })
        .collect()
}

fn with_default_source(device: Device, default: Option<&PrinterDefault>) -> Device {
    let Some(default) = default else {
        return device;
    };
    device.with_source(SourceEvidence {
        source: default.source.clone(),
        kind: SourceKind::Command,
        status: SourceStatus::Success,
        summary: None,
    })
}
