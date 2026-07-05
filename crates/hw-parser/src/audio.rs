use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsoundCardRecord {
    pub index: u32,
    pub id: Option<String>,
    pub name: Option<String>,
    pub detail: Option<String>,
}

pub fn parse_proc_asound_cards(input: &str) -> Vec<AsoundCardRecord> {
    let re = Regex::new(r"^\s*(\d+)\s+\[(.*?)\s*\]:\s*(.*?)\s+-\s+(.*)$").unwrap();
    let mut cards = Vec::new();
    let mut lines = input.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(caps) = re.captures(line) {
            let detail = lines
                .peek()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            cards.push(AsoundCardRecord {
                index: caps[1].parse().unwrap_or(0),
                id: Some(caps[2].trim().to_string()),
                name: Some(caps[4].trim().to_string()),
                detail,
            });
        }
    }
    cards
}
