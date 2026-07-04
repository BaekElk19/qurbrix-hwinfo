use crate::{ProbeContext, ProbeResult};
use async_trait::async_trait;
use hw_model::DeviceKind;

#[async_trait]
pub trait Probe: Send + Sync {
    fn name(&self) -> &'static str;
    fn kinds(&self) -> &'static [DeviceKind];
    async fn probe(&self, ctx: &ProbeContext<'_>) -> ProbeResult;
}
