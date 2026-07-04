use hw_model::{Device, DeviceRef, ScanWarning};

#[derive(Debug, Clone, Default)]
pub struct ProbeResult {
    pub devices: Vec<Device>,
    pub warnings: Vec<ScanWarning>,
    pub consumed: Vec<DeviceRef>,
}

impl ProbeResult {
    pub fn with_devices(devices: Vec<Device>) -> Self {
        Self {
            devices,
            warnings: Vec::new(),
            consumed: Vec::new(),
        }
    }
}
