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
        _ => None,
    }
}

pub fn infer_cpu_vendor_from_name(model_name: &str) -> Option<&'static str> {
    let name = model_name.trim().to_ascii_lowercase();

    if name.contains("loongson") {
        Some("Loongson")
    } else if name.contains("phytium") || name.contains("d2000") {
        Some("Phytium")
    } else if name.contains("kunpeng")
        || name.contains("hisilicon")
        || name.contains("kirin")
        || name.contains("huawei")
    {
        Some("HiSilicon")
    } else if name.contains("zhaoxin") {
        Some("Zhaoxin")
    } else if name.contains("hygon") {
        Some("Hygon")
    } else if name.contains("sunway") {
        Some("Sunway")
    } else if name.contains("intel") {
        Some("Intel")
    } else if name.contains("amd") {
        Some("AMD")
    } else if name.contains("arm") {
        Some("ARM")
    } else {
        None
    }
}
