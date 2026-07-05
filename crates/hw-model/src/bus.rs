use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BusInfo {
    Pci {
        address: String,
        vendor_id: Option<String>,
        device_id: Option<String>,
        subsystem_vendor_id: Option<String>,
        subsystem_device_id: Option<String>,
        class: Option<String>,
    },
    Usb {
        bus: Option<String>,
        device: Option<String>,
        vendor_id: Option<String>,
        product_id: Option<String>,
        interface: Option<String>,
        class: Option<String>,
    },
    Platform {
        path: String,
    },
    Virtual,
    Unknown,
}
