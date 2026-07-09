pub fn normalize_board_vendor(vendor: &str) -> Option<&'static str> {
    let vendor = vendor.trim().to_ascii_lowercase();

    if vendor.contains("loongson") || vendor.contains("龙芯") {
        Some("Loongson")
    } else if vendor.contains("phytium") || vendor.contains("飞腾") {
        Some("Phytium")
    } else if vendor.contains("hygon") || vendor.contains("海光") {
        Some("Hygon")
    } else if vendor.contains("zhaoxin") || vendor.contains("兆芯") {
        Some("Zhaoxin")
    } else if vendor.contains("kunpeng")
        || vendor.contains("hisilicon")
        || vendor.contains("huawei")
        || vendor.contains("鲲鹏")
        || vendor.contains("华为")
    {
        Some("HiSilicon")
    } else if vendor.contains("sunway") || vendor.contains("shenwei") || vendor.contains("申威") {
        Some("Sunway")
    } else if vendor.contains("biostar") || vendor.contains("映泰") {
        Some("BIOSTAR")
    } else if vendor.contains("colorful") || vendor.contains("七彩虹") {
        Some("Colorful")
    } else if vendor.contains("lenovo") {
        Some("Lenovo")
    } else if vendor.contains("dell") {
        Some("Dell")
    } else if vendor.contains("hewlett-packard") || vendor.contains("hp ") || vendor == "hp" {
        Some("HP")
    } else if vendor.contains("asus") || vendor.contains("asustek") {
        Some("ASUS")
    } else if vendor.contains("gigabyte") {
        Some("GIGABYTE")
    } else if vendor.contains("micro-star") || vendor.contains("msi") {
        Some("MSI")
    } else {
        None
    }
}
