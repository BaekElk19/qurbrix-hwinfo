use hw_parser::{parse_lspci_nn_k, parse_lsusb};

#[test]
fn parses_lspci_driver_and_modules() {
    let input = hw_testdata::fixture("pci/lspci-nn-k.txt");
    let records = parse_lspci_nn_k(&input);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].address, "0000:00:1f.3");
    assert_eq!(records[0].class_name.as_deref(), Some("Audio device"));
    assert_eq!(records[0].class_id.as_deref(), Some("0403"));
    assert_eq!(records[0].vendor_id.as_deref(), Some("8086"));
    assert_eq!(records[0].device_id.as_deref(), Some("a348"));
    assert_eq!(records[0].kernel_driver.as_deref(), Some("snd_hda_intel"));
    assert_eq!(
        records[0].kernel_modules,
        vec!["snd_hda_intel", "snd_soc_avs"]
    );
}

#[test]
fn parses_lsusb_basic_records() {
    let input = hw_testdata::fixture("usb/lsusb.txt");
    let records = parse_lsusb(&input);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].bus.as_deref(), Some("001"));
    assert_eq!(records[0].device.as_deref(), Some("004"));
    assert_eq!(records[0].vendor_id.as_deref(), Some("0bda"));
    assert_eq!(records[0].product_id.as_deref(), Some("5689"));
    assert!(records[0]
        .product
        .as_deref()
        .unwrap()
        .contains("Integrated Camera"));
}
