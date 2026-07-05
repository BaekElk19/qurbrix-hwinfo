pub fn normalize_gpu_vendor_id(vendor_id: &str) -> Option<&'static str> {
    match vendor_id
        .trim()
        .trim_start_matches("0x")
        .to_ascii_lowercase()
        .as_str()
    {
        "10de" => Some("NVIDIA"),
        "1002" => Some("AMD"),
        "8086" => Some("Intel"),
        "102b" => Some("Matrox"),
        "1a03" => Some("ASPEED"),
        "15ad" => Some("VMware"),
        "1af4" => Some("VirtIO"),
        "0731" => Some("Jingjia Micro"),
        _ => None,
    }
}

pub fn normalize_gpu_vendor(vendor: &str) -> Option<&'static str> {
    let vendor = vendor.trim().to_ascii_lowercase();

    if vendor.contains("nvidia") {
        Some("NVIDIA")
    } else if vendor.contains("advanced micro devices") || vendor.contains("amd") {
        Some("AMD")
    } else if vendor.contains("intel") {
        Some("Intel")
    } else if vendor.contains("matrox") {
        Some("Matrox")
    } else if vendor.contains("aspeed") {
        Some("ASPEED")
    } else if vendor.contains("vmware") {
        Some("VMware")
    } else if vendor.contains("red hat") || vendor.contains("virtio") {
        Some("VirtIO")
    } else if vendor.contains("loongson") {
        Some("Loongson")
    } else if vendor.contains("jingjia") || vendor.contains("jjm") {
        Some("Jingjia Micro")
    } else if vendor.contains("zhaoxin") {
        Some("Zhaoxin")
    } else if vendor.contains("moore threads") || vendor.contains("mthreads") {
        Some("Moore Threads")
    } else if vendor.contains("innosilicon") {
        Some("Innosilicon")
    } else if vendor.contains("wuhan digital engineering") {
        Some("WDE")
    } else {
        None
    }
}
