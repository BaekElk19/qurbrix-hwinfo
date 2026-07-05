use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CdromProcInfo {
    pub drive_names: Vec<String>,
    pub capabilities: Vec<String>,
}

pub fn parse_proc_cdrom_info(input: &str) -> CdromProcInfo {
    let mut info = CdromProcInfo::default();
    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("drive name:") {
            info.drive_names = rest.split_whitespace().map(ToOwned::to_owned).collect();
        } else if line.starts_with("Can read DVD:") && line.ends_with('1') {
            info.capabilities.push("read-dvd".to_string());
        } else if line.starts_with("Can write CD-R:") && line.ends_with('1') {
            info.capabilities.push("write-cd-r".to_string());
        } else if line.starts_with("Can open tray:") && line.ends_with('1') {
            info.capabilities.push("open-tray".to_string());
        }
    }
    info
}
