use crate::{BusInfo, DeviceKind, DeviceProperties, DriverInfo, ScanWarning, SourceEvidence};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceIdentifier {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceRef {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub kind: DeviceKind,
    pub name: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub bus: Option<BusInfo>,
    pub driver: Option<DriverInfo>,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub capabilities: Vec<String>,
    pub identifiers: Vec<DeviceIdentifier>,
    pub sources: Vec<SourceEvidence>,
    pub warnings: Vec<ScanWarning>,
    pub properties: DeviceProperties,
}

impl Device {
    pub fn new(
        id: impl Into<String>,
        kind: DeviceKind,
        name: impl Into<String>,
        properties: DeviceProperties,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            name: name.into(),
            vendor: None,
            model: None,
            serial: None,
            bus: None,
            driver: None,
            parent_id: None,
            children: Vec::new(),
            capabilities: Vec::new(),
            identifiers: Vec::new(),
            sources: Vec::new(),
            warnings: Vec::new(),
            properties,
        }
    }

    pub fn with_bus(mut self, bus: BusInfo) -> Self {
        self.bus = Some(bus);
        self
    }

    pub fn with_driver(mut self, driver: DriverInfo) -> Self {
        self.driver = Some(driver);
        self
    }

    pub fn with_source(mut self, source: SourceEvidence) -> Self {
        self.sources.push(source);
        self
    }
}
