use crate::{Probe, ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::{
    device_id, Device, DeviceKind, DeviceProperties, PrinterInfo, SourceEvidence, SourceKind,
    SourceStatus,
};
use hw_parser::{parse_lpstat_a, parse_lpstat_v};
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
            if uri_result.is_success() {
                fallback.devices =
                    devices_from_uris(parse_lpstat_v(&uri_result.stdout), &uri_result.source);
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
        let devices = parse_lpstat_a(&status.stdout)
            .into_iter()
            .map(|printer| {
                let uri = uris
                    .iter()
                    .find(|u| u.queue == printer.queue)
                    .and_then(|u| u.device_uri.clone());
                Device::new(
                    device_id::printer(&printer.queue),
                    DeviceKind::Printer,
                    printer.queue.clone(),
                    DeviceProperties::Printer(PrinterInfo {
                        queue_name: Some(printer.queue),
                        accepting: Some(printer.accepting),
                        device_uri: uri,
                        make_model: None,
                        is_default: None,
                    }),
                )
                .with_source(SourceEvidence {
                    source: status.source.clone(),
                    kind: SourceKind::Command,
                    status: SourceStatus::Success,
                    summary: None,
                })
            })
            .collect();
        ProbeResult {
            devices,
            warnings,
            consumed: Vec::new(),
        }
    }
}

fn devices_from_uris(uris: Vec<hw_parser::PrinterUriRecord>, source: &str) -> Vec<Device> {
    uris.into_iter()
        .map(|printer| {
            Device::new(
                device_id::printer(&printer.queue),
                DeviceKind::Printer,
                printer.queue.clone(),
                DeviceProperties::Printer(PrinterInfo {
                    queue_name: Some(printer.queue),
                    accepting: None,
                    device_uri: printer.device_uri,
                    make_model: None,
                    is_default: None,
                }),
            )
            .with_source(SourceEvidence {
                source: source.to_string(),
                kind: SourceKind::Command,
                status: SourceStatus::Success,
                summary: None,
            })
        })
        .collect()
}
