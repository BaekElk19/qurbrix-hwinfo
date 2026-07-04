pub fn clean_hex(value: &str) -> String {
    value.trim().trim_start_matches("0x").to_ascii_lowercase()
}

pub fn split_csv_words(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
