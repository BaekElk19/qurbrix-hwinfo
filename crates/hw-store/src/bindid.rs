use sha1::{Digest, Sha1};

pub const ALGO_ID: &str = "sha1-hex16-v1";

/// 把 (k, Option<v>) 规范成排序后的 "k=v|k=v" 用于组件组合键
pub fn build_kv_key(pairs: &[(&str, Option<&str>)]) -> String {
    let mut v: Vec<(String, String)> = pairs
        .iter()
        .map(|(k, ov)| (k.to_string(), ov.unwrap_or("").trim().to_string()))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v.into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("|")
}

/// 机器 bindid：对组件键排序，"||" 拼接 → SHA1 → 取前16 hex
pub fn calculate_bindid(keys: &[String]) -> String {
    let mut ks = keys.to_vec();
    ks.sort();
    let concat = ks.join("||");
    let mut h = Sha1::new();
    h.update(concat.as_bytes());
    hex::encode(h.finalize())[..16].to_string()
}
