pub fn normalize_cpu_vendor_id(vendor_id: &str) -> Option<&'static str> {
    match vendor_id.trim().to_ascii_lowercase().as_str() {
        "genuineintel" => Some("Intel"),
        "authenticamd" => Some("AMD"),
        "hygongenuine" | "hygon" => Some("Hygon"),
        "centaurhauls" | "shanghai" | "zhaoxin" => Some("Zhaoxin"),
        "loongson" => Some("Loongson"),
        "phytium" => Some("Phytium"),
        "huawei" | "hisilicon" => Some("HiSilicon"),
        "sunway" => Some("Sunway"),
        "arm" | "arm limited" => Some("ARM"),
        _ => None,
    }
}

pub fn infer_cpu_vendor_from_name(model_name: &str) -> Option<&'static str> {
    let name = model_name.trim().to_ascii_lowercase();

    if name.contains("loongson") {
        Some("Loongson")
    } else if name.contains("phytium")
        || name.contains("ft-")
        || name.contains("ft1500")
        || name.contains("ft2000")
        || name.contains("d2000")
        || name.contains("s2500")
        || name.contains("pangu")
    {
        Some("Phytium")
    } else if name.contains("kunpeng")
        || name.contains("hisilicon")
        || name.contains("kirin")
        || name.contains("huawei")
        || name.contains("ascend")
    {
        Some("HiSilicon")
    } else if name.contains("zhaoxin") || name.contains("kx-") {
        Some("Zhaoxin")
    } else if name.contains("hygon") || name.contains("dhyana") {
        Some("Hygon")
    } else if name.contains("sunway") || name.contains("sw-") || name.contains("sw6") {
        Some("Sunway")
    } else if name.contains("intel") {
        Some("Intel")
    } else if name.contains("amd") {
        Some("AMD")
    } else if name.contains("cortex-") || name.contains("armv") || name.contains("arm ") {
        Some("ARM")
    } else {
        None
    }
}

/// Deepin-style Loongson name cleanup: strip trailing " @ ... GHz" and
/// normalise stray whitespace so that "Loongson-3A5000 @ 2500.00MHz" becomes
/// "Loongson-3A5000". Also collapses `Processor rev N` markers that some
/// early aarch64 kernels report on Loongson boards.
pub fn clean_loongson_name(name: &str) -> String {
    let mut trimmed = name.trim();
    if let Some((prefix, _)) = trimmed.split_once(" @ ") {
        trimmed = prefix.trim();
    }
    if let Some(prefix) = trimmed.rsplit_once(" Processor rev ").map(|(p, _)| p) {
        trimmed = prefix.trim();
    }
    trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}
