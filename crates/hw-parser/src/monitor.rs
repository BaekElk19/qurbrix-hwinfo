use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct XrandrMonitorRecord {
    pub connector: String,
    pub connected: bool,
    pub primary: bool,
    pub resolution: Option<String>,
}

pub fn parse_xrandr_query(input: &str) -> Vec<XrandrMonitorRecord> {
    input
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let connector = parts.next()?;
            let state = parts.next()?;
            if state != "connected" && state != "disconnected" {
                return None;
            }
            let rest: Vec<&str> = parts.collect();
            let primary = rest.contains(&"primary");
            let resolution = rest
                .iter()
                .find(|part| part.contains('x') && part.contains('+'))
                .map(|value| value.split('+').next().unwrap_or(value).to_string());
            Some(XrandrMonitorRecord {
                connector: connector.to_string(),
                connected: state == "connected",
                primary,
                resolution,
            })
        })
        .collect()
}
