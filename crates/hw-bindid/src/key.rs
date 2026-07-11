use sha1::{Digest, Sha1};

const PLACEHOLDERS: &[&str] = &[
    "none",
    "n/a",
    "not specified",
    "no asset tag",
    "to be filled by o.e.m.",
    "system serial number",
    "default string",
    "unknown",
];

pub fn normalize_value(value: &str) -> Option<String> {
    let normalized = collapse_whitespace(value);
    if normalized.is_empty() || is_placeholder_value(&normalized) {
        None
    } else {
        Some(normalized)
    }
}

pub fn is_placeholder_value(value: &str) -> bool {
    let normalized = collapse_whitespace(value);
    PLACEHOLDERS
        .iter()
        .any(|placeholder| normalized.eq_ignore_ascii_case(placeholder))
}

pub fn normalize_mac(value: &str) -> Option<String> {
    let mac = normalize_value(value)?.to_ascii_lowercase();
    if mac == "00:00:00:00:00:00" {
        return None;
    }
    let parts = mac.split(':').collect::<Vec<_>>();
    if parts.len() == 6
        && parts
            .iter()
            .all(|part| part.len() == 2 && part.chars().all(|ch| ch.is_ascii_hexdigit()))
    {
        Some(mac)
    } else {
        None
    }
}

pub fn component_key(kind: &str, fields: &[(&str, Option<&str>)]) -> Option<String> {
    let mut normalized = fields
        .iter()
        .filter_map(|(name, value)| {
            normalize_field(name, *value).map(|value| ((*name).to_string(), value))
        })
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return None;
    }
    normalized.sort_by(|left, right| left.0.cmp(&right.0));
    Some(format!(
        "{kind}:{}",
        normalized
            .into_iter()
            .map(|(name, value)| format!("{name}={}", escape_value(&value)))
            .collect::<Vec<_>>()
            .join("|")
    ))
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn escape_value(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('|', "%7C")
        .replace('=', "%3D")
}

fn normalize_field(name: &str, value: Option<&str>) -> Option<String> {
    match name {
        "mac" => normalize_mac(value?),
        _ => normalize_value(value?),
    }
}

pub fn bindid_value(keys: &[String]) -> String {
    let mut keys = keys.to_vec();
    keys.sort();
    let concat = keys.join("||");
    let mut hasher = Sha1::new();
    hasher.update(concat.as_bytes());
    hex::encode(hasher.finalize())[..16].to_string()
}
