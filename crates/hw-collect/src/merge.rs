use hw_model::Device;
use std::collections::BTreeMap;

pub fn dedup_devices(devices: Vec<Device>) -> Vec<Device> {
    let mut by_id: BTreeMap<String, Device> = BTreeMap::new();
    for mut device in devices {
        by_id
            .entry(device.id.clone())
            .and_modify(|existing| {
                existing.sources.append(&mut device.sources);
                existing.warnings.append(&mut device.warnings);
                for capability in device.capabilities.drain(..) {
                    if !existing.capabilities.contains(&capability) {
                        existing.capabilities.push(capability);
                    }
                }
            })
            .or_insert(device);
    }
    by_id.into_values().collect()
}
