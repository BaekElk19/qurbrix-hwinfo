use crate::key::component_key;
use hw_model::{Device, DeviceProperties, NetworkInfo};

pub fn component_keys_from_devices(devices: &[Device]) -> Vec<String> {
    let mut keys = devices
        .iter()
        .filter_map(component_key_from_device)
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn component_key_from_device(device: &Device) -> Option<String> {
    match &device.properties {
        DeviceProperties::System(info) => component_key(
            "system",
            &[
                ("manufacturer", info.manufacturer.as_deref()),
                ("product", info.product_name.as_deref()),
            ],
        ),
        DeviceProperties::Motherboard(info) => component_key(
            "motherboard",
            &[
                ("serial", info.serial.as_deref()),
                ("product", info.product_name.as_deref()),
            ],
        ),
        DeviceProperties::Memory(info) => component_key(
            "memory",
            &[
                ("serial", info.serial.as_deref()),
                ("product", info.part_number.as_deref()),
            ],
        ),
        DeviceProperties::Storage(info) => component_key(
            "storage",
            &[
                ("serial", device.serial.as_deref()),
                ("model", device.model.as_deref().or(info.controller_model.as_deref())),
            ],
        ),
        DeviceProperties::Network(info) => {
            if is_loopback_network(info) {
                return None;
            }
            component_key("network", &[("mac", info.mac.as_deref())])
        }
        DeviceProperties::Gpu(info) => component_key(
            "gpu",
            &[
                ("name", Some(device.name.as_str())),
                ("model", device.model.as_deref().or(info.description.as_deref())),
            ],
        ),
        DeviceProperties::Bios(_)
        | DeviceProperties::Cpu(_)
        | DeviceProperties::Monitor(_)
        | DeviceProperties::Audio(_)
        | DeviceProperties::Bluetooth(_)
        | DeviceProperties::Input(_)
        | DeviceProperties::Camera(_)
        | DeviceProperties::Battery(_)
        | DeviceProperties::Printer(_)
        | DeviceProperties::Cdrom(_)
        | DeviceProperties::Usb(_)
        | DeviceProperties::Pci(_)
        | DeviceProperties::OtherPci(_)
        | DeviceProperties::OtherDevice(_) => None,
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
