pub fn pci(address: &str) -> String {
    format!("pci:{}", address.trim())
}

pub fn usb(
    bus: Option<&str>,
    device: Option<&str>,
    vendor_id: Option<&str>,
    product_id: Option<&str>,
    serial: Option<&str>,
) -> String {
    if let (Some(vendor_id), Some(product_id), Some(serial)) = (vendor_id, product_id, serial) {
        let serial = serial.trim();
        if !serial.is_empty() {
            return format!("usb:{}:{}:{}", vendor_id.trim(), product_id.trim(), serial);
        }
    }
    format!(
        "usb:{}:{}",
        bus.unwrap_or("unknown").trim(),
        device.unwrap_or("unknown").trim()
    )
}

pub fn network(mac: Option<&str>, iface: &str) -> String {
    match mac.map(str::trim).filter(|v| !v.is_empty()) {
        Some(mac) => format!("net:mac:{}", mac),
        None => format!("net:iface:{}", iface.trim()),
    }
}

pub fn storage(wwn: Option<&str>, serial: Option<&str>, node: &str) -> String {
    if let Some(wwn) = wwn.map(str::trim).filter(|v| !v.is_empty()) {
        return format!("storage:wwn:{}", wwn);
    }
    if let Some(serial) = serial.map(str::trim).filter(|v| !v.is_empty()) {
        return format!("storage:serial:{}", serial);
    }
    format!("storage:dev:{}", node.trim())
}

pub fn battery(name: &str) -> String {
    format!("battery:{}", name.trim())
}

pub fn input_event(event: &str) -> String {
    format!("input:event:{}", event.trim())
}

pub fn camera(video_node: &str) -> String {
    format!("camera:{}", video_node.trim())
}

pub fn printer(queue: &str) -> String {
    format!("printer:{}", queue.trim())
}

pub fn other(prefix: &str, value: &str) -> String {
    format!("{}:{}", prefix.trim(), value.trim())
}
