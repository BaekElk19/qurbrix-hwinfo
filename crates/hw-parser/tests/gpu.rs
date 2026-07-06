use hw_parser::{parse_gpu_lspci, parse_lshw_display};

#[test]
fn parses_lshw_display_records() {
    let records = parse_lshw_display(
        "  *-display\n\
              description: VGA compatible controller\n\
              product: Navi 23 [Radeon RX 6600]\n\
              vendor: Advanced Micro Devices, Inc. [AMD/ATI]\n\
              bus info: pci@0000:03:00.0\n\
              configuration: depth=32 driver=amdgpu latency=0\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].product.as_deref(),
        Some("Navi 23 [Radeon RX 6600]")
    );
    assert_eq!(
        records[0].vendor.as_deref(),
        Some("Advanced Micro Devices, Inc. [AMD/ATI]")
    );
    assert_eq!(records[0].bus_info.as_deref(), Some("pci@0000:03:00.0"));
    assert_eq!(records[0].driver.as_deref(), Some("amdgpu"));
}

#[test]
fn gpu_lspci_keeps_display_records() {
    let records = parse_gpu_lspci(
        "03:00.0 Display controller [0380]: Device [1234:5678]\n\
         \tKernel driver in use: acme\n",
    );

    assert_eq!(records.len(), 1);
}
