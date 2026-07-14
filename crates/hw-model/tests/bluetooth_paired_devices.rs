use hw_model::properties::{BluetoothInfo, PairedDeviceInfo};

#[test]
fn paired_device_info_serde_round_trip() {
    let info = BluetoothInfo {
        paired_devices: vec![
            PairedDeviceInfo {
                address: "AA:BB:CC:DD:EE:FF".into(),
                name: "Sony WH-1000XM4".into(),
            },
            PairedDeviceInfo {
                address: "11:22:33:44:55:66".into(),
                name: "Logitech MX Master 3".into(),
            },
        ],
        ..Default::default()
    };
    let json = serde_json::to_string(&info).expect("serialize");
    assert!(
        json.contains(r#""address":"AA:BB:CC:DD:EE:FF""#),
        "serialized address missing: {json}"
    );
    assert!(
        json.contains(r#""name":"Sony WH-1000XM4""#),
        "serialized name missing: {json}"
    );

    let round: BluetoothInfo = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round.paired_devices.len(), 2);
    assert_eq!(round.paired_devices[0].address, "AA:BB:CC:DD:EE:FF");
    assert_eq!(round.paired_devices[0].name, "Sony WH-1000XM4");
    assert_eq!(round.paired_devices[1].address, "11:22:33:44:55:66");
    assert_eq!(round.paired_devices[1].name, "Logitech MX Master 3");
}

#[test]
fn paired_devices_default_is_empty_vec() {
    let info = BluetoothInfo::default();
    assert!(info.paired_devices.is_empty());
    let json = serde_json::to_string(&info).expect("serialize");
    // sanity: default serializes cleanly without panicking; presence of
    // the field itself in the string is enough regardless of ordering
    assert!(
        json.contains(r#""paired_devices":[]"#),
        "unexpected: {json}"
    );
}
