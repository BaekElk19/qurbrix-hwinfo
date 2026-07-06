use hw_parser::parse_lshw_network;

#[test]
fn parses_lshw_network_interfaces() {
    let records = parse_lshw_network(
        "  *-network\n\
              description: Ethernet interface\n\
              product: Ethernet Connection (16) I219-LM\n\
              vendor: Intel Corporation\n\
              bus info: pci@0000:00:1f.6\n\
              logical name: enp0s31f6\n\
              serial: aa:bb:cc:dd:ee:ff\n\
              capacity: 1Gbit/s\n\
              configuration: broadcast=yes driver=e1000e driverversion=6.8.0 firmware=0.8-4\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].logical_name.as_deref(), Some("enp0s31f6"));
    assert_eq!(
        records[0].product.as_deref(),
        Some("Ethernet Connection (16) I219-LM")
    );
    assert_eq!(records[0].vendor.as_deref(), Some("Intel Corporation"));
    assert_eq!(records[0].bus_info.as_deref(), Some("pci@0000:00:1f.6"));
    assert_eq!(records[0].serial.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
    assert_eq!(records[0].capacity_mbps, Some(1000));
    assert_eq!(records[0].driver.as_deref(), Some("e1000e"));
    assert_eq!(records[0].driver_version.as_deref(), Some("6.8.0"));
    assert_eq!(records[0].firmware.as_deref(), Some("0.8-4"));
}
