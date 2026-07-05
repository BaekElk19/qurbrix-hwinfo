use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DeviceKind {
    System,
    Motherboard,
    Bios,
    Cpu,
    Memory,
    Storage,
    Gpu,
    Monitor,
    Network,
    Audio,
    Bluetooth,
    Input,
    Camera,
    Battery,
    Printer,
    Cdrom,
    Usb,
    Pci,
    OtherPci,
    OtherDevice,
}

impl DeviceKind {
    pub const ALL: &'static [DeviceKind] = &[
        DeviceKind::System,
        DeviceKind::Motherboard,
        DeviceKind::Bios,
        DeviceKind::Cpu,
        DeviceKind::Memory,
        DeviceKind::Storage,
        DeviceKind::Gpu,
        DeviceKind::Monitor,
        DeviceKind::Network,
        DeviceKind::Audio,
        DeviceKind::Bluetooth,
        DeviceKind::Input,
        DeviceKind::Camera,
        DeviceKind::Battery,
        DeviceKind::Printer,
        DeviceKind::Cdrom,
        DeviceKind::Usb,
        DeviceKind::Pci,
        DeviceKind::OtherPci,
        DeviceKind::OtherDevice,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            DeviceKind::System => "system",
            DeviceKind::Motherboard => "motherboard",
            DeviceKind::Bios => "bios",
            DeviceKind::Cpu => "cpu",
            DeviceKind::Memory => "memory",
            DeviceKind::Storage => "storage",
            DeviceKind::Gpu => "gpu",
            DeviceKind::Monitor => "monitor",
            DeviceKind::Network => "network",
            DeviceKind::Audio => "audio",
            DeviceKind::Bluetooth => "bluetooth",
            DeviceKind::Input => "input",
            DeviceKind::Camera => "camera",
            DeviceKind::Battery => "battery",
            DeviceKind::Printer => "printer",
            DeviceKind::Cdrom => "cdrom",
            DeviceKind::Usb => "usb",
            DeviceKind::Pci => "pci",
            DeviceKind::OtherPci => "other-pci",
            DeviceKind::OtherDevice => "other-device",
        }
    }
}

impl fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DeviceKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "system" => Ok(DeviceKind::System),
            "motherboard" => Ok(DeviceKind::Motherboard),
            "bios" => Ok(DeviceKind::Bios),
            "cpu" => Ok(DeviceKind::Cpu),
            "memory" => Ok(DeviceKind::Memory),
            "storage" => Ok(DeviceKind::Storage),
            "gpu" => Ok(DeviceKind::Gpu),
            "monitor" => Ok(DeviceKind::Monitor),
            "network" => Ok(DeviceKind::Network),
            "audio" => Ok(DeviceKind::Audio),
            "bluetooth" => Ok(DeviceKind::Bluetooth),
            "input" => Ok(DeviceKind::Input),
            "camera" => Ok(DeviceKind::Camera),
            "battery" => Ok(DeviceKind::Battery),
            "printer" => Ok(DeviceKind::Printer),
            "cdrom" => Ok(DeviceKind::Cdrom),
            "usb" => Ok(DeviceKind::Usb),
            "pci" => Ok(DeviceKind::Pci),
            "other-pci" => Ok(DeviceKind::OtherPci),
            "other-device" => Ok(DeviceKind::OtherDevice),
            other => Err(format!("unsupported device kind: {other}")),
        }
    }
}

impl Serialize for DeviceKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DeviceKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}
