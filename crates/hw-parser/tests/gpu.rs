use hw_parser::{
    parse_dmesg_gpu_vram, parse_glxinfo_basic, parse_gpu_lspci, parse_lshw_display,
    parse_nvidia_settings_videoram, parse_nvidia_smi_memory_csv,
};

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

#[test]
fn parses_dmesg_gpu_vram_records() {
    let records = parse_dmesg_gpu_vram(
        "[    2.123456] [drm] 0000:03:00.0: VRAM: 8192M 0x0000008000000000 - 0x0000009FFFFFFFFF\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].pci_address, "0000:03:00.0");
    assert_eq!(records[0].memory_bytes, 8192 * 1024 * 1024);
}

#[test]
fn parses_nvidia_smi_memory_csv_records() {
    let records = parse_nvidia_smi_memory_csv("00000000:03:00.0, 8192\n");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].pci_address, "0000:03:00.0");
    assert_eq!(records[0].memory_bytes, 8192 * 1024 * 1024);
}

#[test]
fn parses_nvidia_settings_videoram() {
    let memory = parse_nvidia_settings_videoram("Attribute 'VideoRam' (deepin:0.0): 2097152.\n");

    assert_eq!(memory, Some(2097152 * 1024));
}

#[test]
fn parses_glxinfo_basic_renderer_vendor_and_version() {
    let record = parse_glxinfo_basic(
        "name of display: :0\n\
         OpenGL vendor string: Intel\n\
         OpenGL renderer string: Mesa Intel(R) UHD Graphics 620 (KBL GT2)\n\
         OpenGL version string: 4.6 (Compatibility Profile) Mesa 23.1.9\n",
    );

    assert_eq!(
        record.renderer.as_deref(),
        Some("Mesa Intel(R) UHD Graphics 620 (KBL GT2)")
    );
    assert_eq!(record.vendor.as_deref(), Some("Intel"));
    assert_eq!(
        record.version.as_deref(),
        Some("4.6 (Compatibility Profile) Mesa 23.1.9")
    );
}
