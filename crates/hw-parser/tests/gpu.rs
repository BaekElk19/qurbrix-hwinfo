use hw_parser::{
    parse_dmesg_gpu_vram, parse_glxinfo_basic, parse_gpu_lspci, parse_lshw_display,
    parse_nvidia_settings_memory_interface, parse_nvidia_settings_videoram,
    parse_nvidia_smi_memory_csv,
};

#[test]
fn parses_lshw_display_records() {
    let records = parse_lshw_display(
        "  *-display\n\
              description: VGA compatible controller\n\
              product: Navi 23 [Radeon RX 6600]\n\
              vendor: Advanced Micro Devices, Inc. [AMD/ATI]\n\
              version: c7\n\
              bus info: pci@0000:03:00.0\n\
              width: 64 bits\n\
              clock: 33MHz\n\
              capabilities: pm pciexpress msi vga_controller bus_master cap_list rom\n\
              configuration: depth=32 driver=amdgpu latency=0 irq=141\n\
              resources: irq:141 memory:fc000000-fcffffff memory:d0000000-dfffffff ioport:f000(size=256)\n",
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
    assert_eq!(
        records[0].description.as_deref(),
        Some("VGA compatible controller")
    );
    assert_eq!(records[0].version.as_deref(), Some("c7"));
    assert_eq!(records[0].bus_info.as_deref(), Some("pci@0000:03:00.0"));
    assert_eq!(records[0].driver.as_deref(), Some("amdgpu"));
    assert_eq!(records[0].width_bits, Some(64));
    assert_eq!(records[0].clock_mhz, Some(33));
    assert_eq!(records[0].irq.as_deref(), Some("141"));
    assert_eq!(
        records[0].capabilities,
        vec![
            "pm",
            "pciexpress",
            "msi",
            "vga_controller",
            "bus_master",
            "cap_list",
            "rom"
        ]
    );
    assert_eq!(records[0].io_port.as_deref(), Some("f000(size=256)"));
    assert_eq!(
        records[0].mem_address.as_deref(),
        Some("fc000000-fcffffff; d0000000-dfffffff")
    );
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
        "[    2.123456] [drm] 0000:03:00.0: VRAM: 8192M 0x0000008000000000 - 0x0000009FFFFFFFFF\n\
         [    3.123456] jjw 0000:04:00.0: VRAM Size 4096 M\n",
    );

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].pci_address, "0000:03:00.0");
    assert_eq!(records[0].memory_bytes, 8192 * 1024 * 1024);
    assert_eq!(records[1].pci_address, "0000:04:00.0");
    assert_eq!(records[1].memory_bytes, 4096 * 1024 * 1024);
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
fn parses_nvidia_settings_memory_interface() {
    let width = parse_nvidia_settings_memory_interface(
        "Attribute 'GPUMemoryInterface' (deepin:0.0): 256.\n",
    );

    assert_eq!(width, Some(256));
}

#[test]
fn parses_glxinfo_basic_renderer_vendor_and_version() {
    let record = parse_glxinfo_basic(
        "name of display: :0\n\
         OpenGL vendor string: Intel\n\
         OpenGL renderer string: Mesa Intel(R) UHD Graphics 620 (KBL GT2)\n\
         OpenGL version string: 4.6 (Compatibility Profile) Mesa 23.1.9\n\
         OpenGL shading language version string: 4.60\n\
         EGL version string: 1.5\n\
         EGL client APIs: OpenGL OpenGL_ES\n",
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
    assert_eq!(record.glsl_version.as_deref(), Some("4.60"));
    assert_eq!(record.egl_version.as_deref(), Some("1.5"));
    assert_eq!(record.egl_client_apis.as_deref(), Some("OpenGL OpenGL_ES"));
}

#[test]
fn modinfo_version_extracts_first_version_line() {
    let input = hw_testdata::fixture("gpu/modinfo-nvidia.txt");
    assert_eq!(
        hw_parser::parse_modinfo_version(&input).as_deref(),
        Some("550.90.07"),
    );
}

#[test]
fn modinfo_version_returns_none_when_missing() {
    assert_eq!(
        hw_parser::parse_modinfo_version("filename: /foo\nlicense: GPL"),
        None
    );
}
