use anyhow::{anyhow, Result};
use hw_model::{CpuInfo, GpuInfo, MemoryInfo, NetInfo, ParseOutput, StorageInfo};

fn take_first<T>(parts: Vec<ParseOutput<T>>) -> Result<ParseOutput<T>> {
    parts
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no parts to merge"))
}

pub fn merge_cpu(parts: Vec<ParseOutput<CpuInfo>>) -> Result<ParseOutput<CpuInfo>> {
    take_first(parts)
}

pub fn merge_memory(parts: Vec<ParseOutput<MemoryInfo>>) -> Result<ParseOutput<MemoryInfo>> {
    take_first(parts)
}

pub fn merge_storage(parts: Vec<ParseOutput<StorageInfo>>) -> Result<ParseOutput<StorageInfo>> {
    take_first(parts)
}

pub fn merge_gpu(parts: Vec<ParseOutput<GpuInfo>>) -> Result<ParseOutput<GpuInfo>> {
    take_first(parts)
}

pub fn merge_net(parts: Vec<ParseOutput<NetInfo>>) -> Result<ParseOutput<NetInfo>> {
    take_first(parts)
}
