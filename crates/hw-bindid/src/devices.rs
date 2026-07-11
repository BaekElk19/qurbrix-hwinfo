use crate::key::component_key;
use hw_model::{Device, DeviceKind, DeviceProperties, NetworkInfo};

pub fn component_keys_from_devices(devices: &[Device]) -> Vec<String> {
    let mut keys = devices
        .iter()
        .filter_map(component_key_from_device)
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn component_key_from_device(device: &Device) -> Option<String> {
    match (device.kind, &device.properties) {
        (DeviceKind::System, DeviceProperties::System(info)) => component_key(
            "system",
            &[
                ("manufacturer", info.manufacturer.as_deref()),
                ("product", info.product_name.as_deref()),
            ],
        ),
        (DeviceKind::Motherboard, DeviceProperties::Motherboard(info)) => component_key(
            "motherboard",
            &[
                ("serial", info.serial.as_deref()),
                ("product", info.product_name.as_deref()),
            ],
        ),
        (DeviceKind::Memory, DeviceProperties::Memory(info)) => component_key(
            "memory",
            &[
                ("serial", info.serial.as_deref()),
                ("product", info.part_number.as_deref()),
            ],
        ),
        (DeviceKind::Storage, DeviceProperties::Storage(info)) => component_key(
            "storage",
            &[
                ("serial", device.serial.as_deref()),
                (
                    "model",
                    device.model.as_deref().or(info.controller_model.as_deref()),
                ),
            ],
        ),
        (DeviceKind::Network, DeviceProperties::Network(info)) => {
            if is_loopback_network(info) {
                return None;
            }
            component_key("network", &[("mac", info.mac.as_deref())])
        }
        (DeviceKind::Gpu, DeviceProperties::Gpu(info)) => component_key(
            "gpu",
            &[
                ("name", stable_gpu_name(&device.name)),
                (
                    "model",
                    device.model.as_deref().or(info.description.as_deref()),
                ),
            ],
        ),
        _ => None,
    }
}

fn is_loopback_network(info: &NetworkInfo) -> bool {
    info.interface
        .as_deref()
        .map(str::trim)
        .is_some_and(|interface| interface == "lo")
        || info
            .network_type
            .as_deref()
            .map(str::trim)
            .is_some_and(|network_type| network_type.eq_ignore_ascii_case("loopback"))
}

fn stable_gpu_name(name: &str) -> Option<&str> {
    let name = name.trim();
    if is_generic_gpu_name(name) {
        None
    } else {
        Some(name)
    }
}

fn is_generic_gpu_name(name: &str) -> bool {
    if name.eq_ignore_ascii_case("intel")
        || name.eq_ignore_ascii_case("amd")
        || name.eq_ignore_ascii_case("nvidia")
    {
        return true;
    }

    name.strip_prefix("GPU ")
        .is_some_and(|suffix| suffix.chars().any(|ch| ch == ':'))
}
