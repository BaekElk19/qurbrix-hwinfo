use hw_bindid::key::{
    bindid_value, component_key, is_placeholder_value, normalize_mac, normalize_value,
};

#[test]
fn normalizes_values_for_component_keys() {
    assert_eq!(
        normalize_value("  GEIT   Computer  "),
        Some("GEIT Computer".to_string())
    );
    assert_eq!(normalize_value("Unknown"), None);
    assert_eq!(normalize_value("To Be Filled By O.E.M."), None);
    assert_eq!(normalize_value(""), None);
}

#[test]
fn recognizes_placeholder_values_case_insensitively() {
    assert!(is_placeholder_value("unknown"));
    assert!(is_placeholder_value("N/A"));
    assert!(is_placeholder_value("System Serial Number"));
    assert!(is_placeholder_value("To   Be Filled By O.E.M."));
    assert!(!is_placeholder_value("UT6619-FC2"));
}

#[test]
fn normalizes_valid_mac_and_rejects_invalid_mac() {
    assert_eq!(
        normalize_mac("AA:BB:CC:DD:EE:FF"),
        Some("aa:bb:cc:dd:ee:ff".to_string())
    );
    assert_eq!(normalize_mac("00:00:00:00:00:00"), None);
    assert_eq!(normalize_mac(""), None);
}

#[test]
fn component_key_sorts_fields_by_name_and_drops_empty_values() {
    let key = component_key(
        "motherboard",
        &[("serial", Some(" SN123 ")), ("product", Some(" Board X "))],
    )
    .unwrap();
    assert_eq!(key, "motherboard:product=Board X|serial=SN123");
}

#[test]
fn component_key_returns_none_when_no_fields_survive() {
    let key = component_key("network", &[("mac", Some("00:00:00:00:00:00"))]);
    assert_eq!(key, None);
}

#[test]
fn component_key_escapes_value_delimiters() {
    let key = component_key(
        "memory",
        &[("serial", Some("A||network:mac=aa:bb:cc:dd:ee:ff"))],
    )
    .unwrap();
    assert_ne!(key, "memory:serial=A||network:mac=aa:bb:cc:dd:ee:ff");
    assert_eq!(key, "memory:serial=A%7C%7Cnetwork:mac%3Daa:bb:cc:dd:ee:ff");
}

#[test]
fn component_key_escapes_equals_and_percent_in_values() {
    let key = component_key("storage", &[("serial", Some("asset=100%"))]).unwrap();
    assert_eq!(key, "storage:serial=asset%3D100%25");
}

#[test]
fn bindid_value_is_order_stable_sha1_hex16() {
    let mut keys = vec![
        "network:mac=aa:bb:cc:dd:ee:ff".to_string(),
        "system:manufacturer=GEIT|product=UT6619-FC2".to_string(),
        "storage:model=Disk X|serial=DISK123".to_string(),
    ];
    let first = bindid_value(&keys);
    keys.reverse();
    let second = bindid_value(&keys);
    assert_eq!(first, second);
    assert_eq!(first, "14924ca00dd7c61e");
    assert_eq!(first.len(), 16);
    assert!(first.chars().all(|ch| ch.is_ascii_hexdigit()));
}
