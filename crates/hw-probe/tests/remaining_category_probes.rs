use hw_model::{BusInfo, DeviceKind, DeviceProperties, DriverStatus, SourceKind, SourceStatus};
use hw_probe::{BiosProbe, GpuProbe, MemoryProbe, MonitorProbe, Probe, ProbeContext};
use hw_source::FakeSourceRunner;
use std::{path::PathBuf, time::Duration};

#[tokio::test]
async fn memory_probe_outputs_dimm_devices() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "memory"],
        "Memory Device\n\tSize: 16 GB\n\tLocator: ChannelA-DIMM0\n\tManufacturer: Samsung\n\tSerial Number: ABCD\n\tPart Number: M471A2K43\n\tType: DDR4\n\tSpeed: 3200 MT/s\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;
    assert_eq!(result.devices[0].kind, DeviceKind::Memory);
}

#[tokio::test]
async fn memory_probe_uses_proc_meminfo_when_dmidecode_is_missing() {
    let runner =
        FakeSourceRunner::new().with_file("/proc/meminfo", "MemTotal:       16384000 kB\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Memory);
    match &result.devices[0].properties {
        DeviceProperties::Memory(memory) => {
            assert_eq!(memory.size_bytes, Some(16384000 * 1024));
        }
        other => panic!("expected memory properties, got {other:?}"),
    }
    assert!(result.devices[0].sources.iter().any(|source| {
        source.source == "/proc/meminfo"
            && source.kind == SourceKind::Procfs
            && source.status == SourceStatus::Success
    }));
    assert!(result.warnings.iter().any(|warning| {
        warning.code == "source_missing" && warning.source.as_deref() == Some("dmidecode -t memory")
    }));
    assert!(result.warnings.iter().any(|warning| {
        warning.code == "source_missing" && warning.source.as_deref() == Some("lshw -class memory")
    }));
}

#[tokio::test]
async fn memory_probe_uses_lshw_when_dmidecode_is_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "lshw",
        ["-class", "memory"],
        "*-memory\n\
             description: System Memory\n\
           *-bank:0\n\
                description: SODIMM DDR4 Synchronous 3200 MHz (0.3 ns)\n\
                product: M471A2K43CB1-CTD\n\
                vendor: Samsung\n\
                serial: ABCD1234\n\
                slot: ChannelA-DIMM0\n\
                size: 8GiB\n\
                clock: 3200MHz (0.3ns)\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].kind, DeviceKind::Memory);
    match &result.devices[0].properties {
        DeviceProperties::Memory(memory) => {
            assert_eq!(memory.size_bytes, Some(8 * 1024 * 1024 * 1024));
            assert_eq!(memory.vendor.as_deref(), Some("Samsung"));
            assert_eq!(memory.memory_type.as_deref(), Some("DDR4"));
            assert_eq!(memory.speed_mtps, Some(3200));
            assert_eq!(memory.locator.as_deref(), Some("ChannelA-DIMM0"));
            assert_eq!(memory.serial.as_deref(), Some("ABCD1234"));
            assert_eq!(memory.part_number.as_deref(), Some("M471A2K43CB1-CTD"));
        }
        other => panic!("expected memory properties, got {other:?}"),
    }
    assert!(result.devices[0].sources.iter().any(|source| {
        source.source == "lshw -class memory"
            && source.kind == SourceKind::Command
            && source.status == SourceStatus::Success
    }));
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("dmidecode -t memory")
    );
}

#[tokio::test]
async fn memory_probe_uses_lshw_when_dmidecode_parses_empty() {
    let runner = FakeSourceRunner::new()
        .with_command("dmidecode", ["-t", "memory"], "")
        .with_command(
            "lshw",
            ["-class", "memory"],
            "*-memory\n\
                 description: System Memory\n\
               *-bank:0\n\
                    description: SODIMM DDR5 Synchronous 4800 MHz\n\
                    product: HMCG66AGBSA092N\n\
                    vendor: SK Hynix\n\
                    serial: 12345678\n\
                    slot: ChannelB-DIMM0\n\
                    size: 16GiB\n\
                    clock: 4800MHz\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Memory(memory) => {
            assert_eq!(memory.size_bytes, Some(16 * 1024 * 1024 * 1024));
            assert_eq!(memory.vendor.as_deref(), Some("SK Hynix"));
            assert_eq!(memory.memory_type.as_deref(), Some("DDR5"));
            assert_eq!(memory.speed_mtps, Some(4800));
            assert_eq!(memory.locator.as_deref(), Some("ChannelB-DIMM0"));
        }
        other => panic!("expected memory properties, got {other:?}"),
    }
    assert!(result.warnings.iter().any(|warning| {
        warning.code == "source_empty" && warning.source.as_deref() == Some("dmidecode -t memory")
    }));
}

#[tokio::test]
async fn memory_probe_uses_edac_sysfs_when_command_sources_are_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/devices/system/edac/mc/mc*",
            vec![PathBuf::from("/sys/devices/system/edac/mc/mc0")],
        )
        .with_glob(
            "/sys/devices/system/edac/mc/mc0/dimm*",
            vec![
                PathBuf::from("/sys/devices/system/edac/mc/mc0/dimm0"),
                PathBuf::from("/sys/devices/system/edac/mc/mc0/dimm1"),
            ],
        )
        .with_file(
            "/sys/devices/system/edac/mc/mc0/dimm0/dimm_label",
            "ChannelA-DIMM0\n",
        )
        .with_file(
            "/sys/devices/system/edac/mc/mc0/dimm0/dimm_mem_type",
            "DDR4\n",
        )
        .with_file("/sys/devices/system/edac/mc/mc0/dimm0/size", "8192\n")
        .with_file(
            "/sys/devices/system/edac/mc/mc0/dimm1/dimm_label",
            "ChannelB-DIMM0\n",
        )
        .with_file(
            "/sys/devices/system/edac/mc/mc0/dimm1/dimm_mem_type",
            "DDR4\n",
        )
        .with_file("/sys/devices/system/edac/mc/mc0/dimm1/size", "8192\n")
        .with_file("/proc/meminfo", "MemTotal:       16384000 kB\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 2);
    match &result.devices[0].properties {
        DeviceProperties::Memory(memory) => {
            assert_eq!(memory.size_bytes, Some(8192 * 1024 * 1024));
            assert_eq!(memory.memory_type.as_deref(), Some("DDR4"));
            assert_eq!(memory.locator.as_deref(), Some("ChannelA-DIMM0"));
        }
        other => panic!("expected memory properties, got {other:?}"),
    }
    assert!(result.devices[0].sources.iter().any(|source| {
        source.source == "/sys/devices/system/edac/mc/mc0/dimm0"
            && source.kind == SourceKind::Sysfs
            && source.status == SourceStatus::Success
    }));
    assert!(result.warnings.iter().any(|warning| {
        warning.code == "source_missing" && warning.source.as_deref() == Some("dmidecode -t memory")
    }));
    assert!(result.warnings.iter().any(|warning| {
        warning.code == "source_missing" && warning.source.as_deref() == Some("lshw -class memory")
    }));
}

#[tokio::test]
async fn memory_probe_uses_spd_decode_dimms_when_command_sources_are_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "decode-dimms",
        std::iter::empty::<&str>(),
        "Decoding EEPROM: /sys/bus/i2c/drivers/eeprom/0-0050\n\
         Guessing DIMM is in                              bank 1\n\
         ---=== SPD EEPROM Information ===---\n\
         Fundamental Memory type                         DDR4 SDRAM\n\
         ---=== Memory Characteristics ===---\n\
         Maximum module speed                            3200 MT/s (PC4-25600)\n\
         Size                                            8192 MB\n\
         ---=== Manufacturer Data ===---\n\
         Module Manufacturer                             Samsung\n\
         Assembly Serial Number                          12345678\n\
         Part Number                                     M471A1K43DB1-CWE\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Memory(memory) => {
            assert_eq!(memory.size_bytes, Some(8192 * 1024 * 1024));
            assert_eq!(memory.vendor.as_deref(), Some("Samsung"));
            assert_eq!(memory.memory_type.as_deref(), Some("DDR4 SDRAM"));
            assert_eq!(memory.speed_mtps, Some(3200));
            assert_eq!(memory.locator.as_deref(), Some("bank 1"));
            assert_eq!(memory.serial.as_deref(), Some("12345678"));
            assert_eq!(memory.part_number.as_deref(), Some("M471A1K43DB1-CWE"));
        }
        other => panic!("expected memory properties, got {other:?}"),
    }
    assert!(result.devices[0].sources.iter().any(|source| {
        source.source == "decode-dimms"
            && source.kind == SourceKind::Command
            && source.status == SourceStatus::Success
    }));
}

#[tokio::test]
async fn memory_probe_uses_raw_spd_eeprom_when_command_sources_are_missing() {
    let spd_path = PathBuf::from("/sys/bus/i2c/drivers/ee1004/0-0050/eeprom");
    let runner = FakeSourceRunner::new()
        .with_glob("/sys/bus/i2c/drivers/eeprom/*/eeprom", Vec::new())
        .with_glob(
            "/sys/bus/i2c/drivers/ee1004/*/eeprom",
            vec![spd_path.clone()],
        )
        .with_file_bytes(spd_path.clone(), ddr4_spd_eeprom());
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = MemoryProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Memory(memory) => {
            assert_eq!(memory.size_bytes, Some(8 * 1024 * 1024 * 1024));
            assert_eq!(memory.memory_type.as_deref(), Some("DDR4 SDRAM"));
            assert_eq!(memory.speed_mtps, Some(3200));
            assert_eq!(memory.locator.as_deref(), Some("0-0050"));
            assert_eq!(memory.serial.as_deref(), Some("12345678"));
            assert_eq!(memory.part_number.as_deref(), Some("M471A1K43DB1-CWE"));
        }
        other => panic!("expected memory properties, got {other:?}"),
    }
    assert!(result.devices[0].sources.iter().any(|source| {
        source.source == spd_path.display().to_string()
            && source.kind == SourceKind::Sysfs
            && source.status == SourceStatus::Success
    }));
}

#[tokio::test]
async fn bios_probe_outputs_bios_and_motherboard_devices() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "0,1,2,3"],
        "BIOS Information\n\tVendor: LENOVO\n\tVersion: N2IET98W\n\tRelease Date: 01/01/2026\nBase Board Information\n\tManufacturer: LENOVO\n\tProduct Name: 20XX\n\tSerial Number: BOARD123\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;
    assert!(result.devices.iter().any(|d| d.kind == DeviceKind::Bios));
    assert!(result
        .devices
        .iter()
        .any(|d| d.kind == DeviceKind::Motherboard));
}

fn ddr4_spd_eeprom() -> Vec<u8> {
    let mut bytes = vec![0; 384];
    bytes[2] = 0x0c;
    bytes[4] = 0x05;
    bytes[12] = 0x01;
    bytes[13] = 0x03;
    bytes[18] = 0x05;
    bytes[125] = 0x00;
    bytes[126] = 0x00;
    bytes[323] = 0x12;
    bytes[324] = 0x34;
    bytes[325] = 0x56;
    bytes[326] = 0x78;
    bytes[329..347].copy_from_slice(b"M471A1K43DB1-CWE  ");
    bytes
}

#[tokio::test]
async fn bios_probe_preserves_baseboard_extended_fields() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "0,1,2,3"],
        "BIOS Information\n\
             \tVendor: LENOVO\n\
             \tVersion: N2IET98W\n\
             Base Board Information\n\
             \tManufacturer: LENOVO\n\
             \tProduct Name: 20XX\n\
             \tVersion: SDK0T76530 WIN\n\
             \tSerial Number: BOARD123\n\
             \tAsset Tag: Not Available\n\
             \tLocation In Chassis: Default string\n\
             \tChassis Handle: 0x0003\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;

    let board = result
        .devices
        .iter()
        .find(|device| device.kind == DeviceKind::Motherboard)
        .expect("expected motherboard device");
    match &board.properties {
        DeviceProperties::Motherboard(info) => {
            assert_eq!(info.version.as_deref(), Some("SDK0T76530 WIN"));
            assert_eq!(info.asset_tag.as_deref(), Some("Not Available"));
            assert_eq!(info.location_in_chassis.as_deref(), Some("Default string"));
            assert_eq!(info.chassis_handle.as_deref(), Some("0x0003"));
        }
        other => panic!("expected motherboard properties, got {other:?}"),
    }
}

#[tokio::test]
async fn bios_probe_preserves_chassis_information_fields() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "0,1,2,3"],
        "BIOS Information\n\
             \tVendor: LENOVO\n\
             \tVersion: N2IET98W\n\
             Base Board Information\n\
             \tManufacturer: LENOVO\n\
             \tProduct Name: 20XX\n\
             \tSerial Number: BOARD123\n\
             Chassis Information\n\
             \tManufacturer: LENOVO\n\
             \tType: Notebook\n\
             \tVersion: ThinkPad\n\
             \tSerial Number: CHASSIS123\n\
             \tAsset Tag: ASSET456\n\
             \tBoot-up State: Safe\n\
             \tPower Supply State: Safe\n\
             \tThermal State: Safe\n\
             \tSecurity Status: None\n\
             \tOEM Information: 0x00000000\n\
             \tHeight: Unspecified\n\
             \tNumber Of Power Cords: 1\n\
             \tContained Elements: 0\n\
             \tSKU Number: SKU123\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;

    let board = result
        .devices
        .iter()
        .find(|device| device.kind == DeviceKind::Motherboard)
        .expect("expected motherboard device");
    match &board.properties {
        DeviceProperties::Motherboard(info) => {
            assert_eq!(info.chassis_manufacturer.as_deref(), Some("LENOVO"));
            assert_eq!(info.chassis_type.as_deref(), Some("Notebook"));
            assert_eq!(info.chassis_version.as_deref(), Some("ThinkPad"));
            assert_eq!(info.chassis_serial.as_deref(), Some("CHASSIS123"));
            assert_eq!(info.chassis_asset_tag.as_deref(), Some("ASSET456"));
            assert_eq!(info.chassis_boot_up_state.as_deref(), Some("Safe"));
            assert_eq!(info.chassis_power_supply_state.as_deref(), Some("Safe"));
            assert_eq!(info.chassis_thermal_state.as_deref(), Some("Safe"));
            assert_eq!(info.chassis_security_status.as_deref(), Some("None"));
            assert_eq!(info.chassis_oem_information.as_deref(), Some("0x00000000"));
            assert_eq!(info.chassis_height.as_deref(), Some("Unspecified"));
            assert_eq!(info.chassis_power_cords.as_deref(), Some("1"));
            assert_eq!(info.chassis_contained_elements.as_deref(), Some("0"));
            assert_eq!(info.chassis_sku_number.as_deref(), Some("SKU123"));
        }
        other => panic!("expected motherboard properties, got {other:?}"),
    }
}

#[tokio::test]
async fn bios_probe_preserves_physical_memory_array_fields() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "dmidecode",
            ["-t", "0,1,2,3"],
            "BIOS Information\n\
                 \tVendor: LENOVO\n\
                 \tVersion: N2IET98W\n\
                 Base Board Information\n\
                 \tManufacturer: LENOVO\n\
                 \tProduct Name: 20XX\n\
                 \tSerial Number: BOARD123\n",
        )
        .with_command(
            "dmidecode",
            ["-t", "16"],
            "Physical Memory Array\n\
                 \tLocation: System Board Or Motherboard\n\
                 \tUse: System Memory\n\
                 \tError Correction Type: None\n\
                 \tMaximum Capacity: 64 GB\n\
                 \tError Information Handle: Not Provided\n\
                 \tNumber Of Devices: 2\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;

    let board = result
        .devices
        .iter()
        .find(|device| device.kind == DeviceKind::Motherboard)
        .expect("expected motherboard device");
    assert!(board.sources.iter().any(|source| {
        source.kind == SourceKind::Command && source.source == "dmidecode -t 16"
    }));
    match &board.properties {
        DeviceProperties::Motherboard(info) => {
            assert_eq!(
                info.memory_array_location.as_deref(),
                Some("System Board Or Motherboard")
            );
            assert_eq!(info.memory_array_use.as_deref(), Some("System Memory"));
            assert_eq!(
                info.memory_array_error_correction_type.as_deref(),
                Some("None")
            );
            assert_eq!(info.memory_array_maximum_capacity.as_deref(), Some("64 GB"));
            assert_eq!(
                info.memory_array_error_information_handle.as_deref(),
                Some("Not Provided")
            );
            assert_eq!(info.memory_array_number_of_devices.as_deref(), Some("2"));
        }
        other => panic!("expected motherboard properties, got {other:?}"),
    }
}

#[tokio::test]
async fn bios_probe_preserves_bios_language_fields() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "dmidecode",
            ["-t", "0,1,2,3"],
            "BIOS Information\n\
                 \tVendor: LENOVO\n\
                 \tVersion: N2IET98W\n\
                 \tRelease Date: 01/01/2026\n",
        )
        .with_command(
            "dmidecode",
            ["-t", "13"],
            "BIOS Language Information\n\
                 \tLanguage Description Format: Long\n\
                 \tInstallable Languages: 1\n\
                 \t\ten|US|iso8859-1\n\
                 \tCurrently Installed Language: en|US|iso8859-1\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;

    let bios = result
        .devices
        .iter()
        .find(|device| device.kind == DeviceKind::Bios)
        .expect("expected bios device");
    assert!(bios.sources.iter().any(|source| {
        source.kind == SourceKind::Command && source.source == "dmidecode -t 13"
    }));
    match &bios.properties {
        DeviceProperties::Bios(info) => {
            assert_eq!(info.language_description_format.as_deref(), Some("Long"));
            assert_eq!(info.installable_languages, ["en|US|iso8859-1"]);
            assert_eq!(
                info.currently_installed_language.as_deref(),
                Some("en|US|iso8859-1")
            );
        }
        other => panic!("expected bios properties, got {other:?}"),
    }
}

#[tokio::test]
async fn bios_probe_does_not_emit_generic_devices_for_empty_dmi_output() {
    let runner = FakeSourceRunner::new().with_command("dmidecode", ["-t", "0,1,2,3"], "");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_empty");
}

#[tokio::test]
async fn bios_probe_uses_sysfs_dmi_when_dmidecode_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_file("/sys/class/dmi/id/bios_vendor", "LENOVO\n")
        .with_file("/sys/class/dmi/id/bios_version", "N2IET98W\n")
        .with_file("/sys/class/dmi/id/bios_date", "01/01/2026\n")
        .with_file("/sys/class/dmi/id/board_vendor", "LENOVO\n")
        .with_file("/sys/class/dmi/id/board_name", "20XX\n")
        .with_file("/sys/class/dmi/id/board_version", "SDK0T76530 WIN\n")
        .with_file("/sys/class/dmi/id/board_serial", "BOARD123\n")
        .with_file("/sys/class/dmi/id/board_asset_tag", "ASSET456\n")
        .with_file("/sys/class/dmi/id/chassis_vendor", "LENOVO\n")
        .with_file("/sys/class/dmi/id/chassis_type", "10\n")
        .with_file("/sys/class/dmi/id/chassis_version", "ThinkPad\n")
        .with_file("/sys/class/dmi/id/chassis_serial", "CHASSIS123\n")
        .with_file("/sys/class/dmi/id/chassis_asset_tag", "CHASSET456\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 2);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some("dmidecode -t 0,1,2,3")
    );

    let bios = result
        .devices
        .iter()
        .find(|device| device.kind == DeviceKind::Bios)
        .expect("expected bios device");
    assert_eq!(bios.name, "N2IET98W");
    assert!(bios.sources.iter().any(|source| {
        source.kind == SourceKind::Sysfs && source.source == "/sys/class/dmi/id"
    }));
    match &bios.properties {
        DeviceProperties::Bios(info) => {
            assert_eq!(info.vendor.as_deref(), Some("LENOVO"));
            assert_eq!(info.version.as_deref(), Some("N2IET98W"));
            assert_eq!(info.release_date.as_deref(), Some("01/01/2026"));
        }
        other => panic!("expected bios properties, got {other:?}"),
    }

    let board = result
        .devices
        .iter()
        .find(|device| device.kind == DeviceKind::Motherboard)
        .expect("expected motherboard device");
    assert_eq!(board.name, "20XX");
    assert!(board.sources.iter().any(|source| {
        source.kind == SourceKind::Sysfs && source.source == "/sys/class/dmi/id"
    }));
    match &board.properties {
        DeviceProperties::Motherboard(info) => {
            assert_eq!(info.manufacturer.as_deref(), Some("LENOVO"));
            assert_eq!(info.product_name.as_deref(), Some("20XX"));
            assert_eq!(info.version.as_deref(), Some("SDK0T76530 WIN"));
            assert_eq!(info.serial.as_deref(), Some("BOARD123"));
            assert_eq!(info.asset_tag.as_deref(), Some("ASSET456"));
            assert_eq!(info.chassis_manufacturer.as_deref(), Some("LENOVO"));
            assert_eq!(info.chassis_type.as_deref(), Some("Notebook"));
            assert_eq!(info.chassis_version.as_deref(), Some("ThinkPad"));
            assert_eq!(info.chassis_serial.as_deref(), Some("CHASSIS123"));
            assert_eq!(info.chassis_asset_tag.as_deref(), Some("CHASSET456"));
        }
        other => panic!("expected motherboard properties, got {other:?}"),
    }
}

#[tokio::test]
async fn bios_probe_reads_uefi_secure_boot_state() {
    let secure_boot =
        PathBuf::from("/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c");
    let runner = FakeSourceRunner::new()
        .with_command(
            "dmidecode",
            ["-t", "0,1,2,3"],
            "BIOS Information\n\tVendor: LENOVO\n\tVersion: N2IET98W\n\tRelease Date: 01/01/2026\n",
        )
        .with_glob(
            "/sys/firmware/efi",
            vec![PathBuf::from("/sys/firmware/efi")],
        )
        .with_glob(
            "/sys/firmware/efi/efivars/SecureBoot-*",
            vec![secure_boot.clone()],
        )
        .with_file_bytes(secure_boot, vec![0, 0, 0, 0, 1]);
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;

    let bios = result
        .devices
        .iter()
        .find(|device| device.kind == DeviceKind::Bios)
        .expect("expected bios device");
    match &bios.properties {
        DeviceProperties::Bios(info) => {
            assert_eq!(info.firmware_type.as_deref(), Some("uefi"));
            assert_eq!(info.secure_boot.as_deref(), Some("enabled"));
        }
        other => panic!("expected bios properties, got {other:?}"),
    }
}

#[tokio::test]
async fn bios_probe_marks_legacy_bios_when_efi_is_absent() {
    let runner = FakeSourceRunner::new().with_command(
        "dmidecode",
        ["-t", "0,1,2,3"],
        "BIOS Information\n\tVendor: LENOVO\n\tVersion: N2IET98W\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = BiosProbe.probe(&ctx).await;

    let bios = result
        .devices
        .iter()
        .find(|device| device.kind == DeviceKind::Bios)
        .expect("expected bios device");
    match &bios.properties {
        DeviceProperties::Bios(info) => {
            assert_eq!(info.firmware_type.as_deref(), Some("bios"));
            assert_eq!(info.secure_boot, None);
        }
        other => panic!("expected bios properties, got {other:?}"),
    }
}

#[tokio::test]
async fn gpu_and_monitor_probes_output_devices() {
    let runner = FakeSourceRunner::new()
        .with_command("lspci", ["-nn", "-k"], "00:02.0 VGA compatible controller [0300]: Intel Corporation UHD Graphics [8086:9a49]\n\tKernel driver in use: i915\n")
        .with_command("xrandr", ["--query"], "eDP-1 connected primary 1920x1080+0+0\n   1920x1080     60.00*+\nHDMI-1 disconnected\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let gpu_result = GpuProbe.probe(&ctx).await;
    assert_eq!(gpu_result.devices[0].kind, DeviceKind::Gpu);
    match &gpu_result.devices[0].properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.vendor.as_deref(), Some("Intel"));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert_eq!(
        MonitorProbe.probe(&ctx).await.devices[0].kind,
        DeviceKind::Monitor
    );
}

#[tokio::test]
async fn gpu_probe_preserves_unknown_raw_device_description_as_vendor() {
    let runner = FakeSourceRunner::new().with_command(
        "lspci",
        ["-nn", "-k"],
        "00:03.0 Display controller [0380]: Acme Accelerant 9000 [1234:5678]\n\tKernel driver in use: acme\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let gpu_result = GpuProbe.probe(&ctx).await;

    assert_eq!(gpu_result.devices[0].kind, DeviceKind::Gpu);
    match &gpu_result.devices[0].properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.vendor.as_deref(), Some("Acme Accelerant 9000"));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn gpu_probe_normalizes_vendor_from_numeric_vendor_id_when_text_is_generic() {
    let runner = FakeSourceRunner::new().with_command(
        "lspci",
        ["-nn", "-k"],
        "00:03.0 Display controller [0380]: Device [1002:1638]\n\tKernel driver in use: amdgpu\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let gpu_result = GpuProbe.probe(&ctx).await;

    assert_eq!(gpu_result.devices[0].kind, DeviceKind::Gpu);
    match &gpu_result.devices[0].properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.vendor.as_deref(), Some("AMD"));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn gpu_probe_enriches_human_readable_lshw_display_fields() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "03:00.0 Display controller [0380]: Device [1234:5678]\n",
        )
        .with_command(
            "lshw",
            ["-class", "display"],
            "  *-display\n\
                  description: VGA compatible controller\n\
                  product: Jingjia JM9 Series Graphics Adapter\n\
                  vendor: Jingjia Micro\n\
                  bus info: pci@0000:03:00.0\n\
                  configuration: driver=jm9 latency=0\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.name, "Jingjia JM9 Series Graphics Adapter");
    assert_eq!(device.vendor.as_deref(), Some("Jingjia Micro"));
    assert_eq!(
        device
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("jm9")
    );
    assert!(
        device
            .sources
            .iter()
            .any(|source| source.kind == SourceKind::Command
                && source.source == "lshw -class display")
    );
    match &device.properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.vendor.as_deref(), Some("Jingjia Micro"));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn gpu_probe_enriches_single_gpu_from_glxinfo_basic() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "00:02.0 VGA compatible controller [0300]: Intel Corporation UHD Graphics [8086:9a49]\n\tKernel driver in use: i915\n",
        )
        .with_command(
            "glxinfo",
            ["-B"],
            "OpenGL vendor string: Intel\n\
             OpenGL renderer string: Mesa Intel(R) UHD Graphics 620 (KBL GT2)\n\
             OpenGL version string: 4.6 (Compatibility Profile) Mesa 23.1.9\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.name, "Mesa Intel(R) UHD Graphics 620 (KBL GT2)");
    assert_eq!(
        device.model.as_deref(),
        Some("Mesa Intel(R) UHD Graphics 620 (KBL GT2)")
    );
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "glxinfo -B"));
    match &device.properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(
                gpu.renderer.as_deref(),
                Some("Mesa Intel(R) UHD Graphics 620 (KBL GT2)")
            );
            assert_eq!(gpu.opengl_vendor.as_deref(), Some("Intel"));
            assert_eq!(
                gpu.opengl_version.as_deref(),
                Some("4.6 (Compatibility Profile) Mesa 23.1.9")
            );
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn gpu_probe_enriches_unique_matching_gpu_from_glxinfo_basic() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "00:02.0 VGA compatible controller [0300]: Intel Corporation UHD Graphics [8086:9a49]\n\tKernel driver in use: i915\n03:00.0 VGA compatible controller [0300]: NVIDIA Corporation GA104 [GeForce RTX 3070] [10de:2484]\n\tKernel driver in use: nvidia\n",
        )
        .with_command(
            "glxinfo",
            ["-B"],
            "OpenGL vendor string: NVIDIA Corporation\n\
             OpenGL renderer string: NVIDIA GeForce RTX 3070/PCIe/SSE2\n\
             OpenGL version string: 4.6.0 NVIDIA 535.154.05\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 2);
    let intel = &result.devices[0];
    let nvidia = &result.devices[1];

    match &intel.properties {
        DeviceProperties::Gpu(gpu) => assert_eq!(gpu.renderer, None),
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert!(!intel
        .sources
        .iter()
        .any(|source| source.source == "glxinfo -B"));

    assert_eq!(nvidia.name, "NVIDIA GeForce RTX 3070/PCIe/SSE2");
    assert_eq!(
        nvidia.model.as_deref(),
        Some("NVIDIA GeForce RTX 3070/PCIe/SSE2")
    );
    match &nvidia.properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(
                gpu.renderer.as_deref(),
                Some("NVIDIA GeForce RTX 3070/PCIe/SSE2")
            );
            assert_eq!(gpu.opengl_vendor.as_deref(), Some("NVIDIA Corporation"));
            assert_eq!(
                gpu.opengl_version.as_deref(),
                Some("4.6.0 NVIDIA 535.154.05")
            );
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert!(nvidia
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "glxinfo -B"));
}

#[tokio::test]
async fn gpu_probe_skips_glxinfo_for_ambiguous_matching_gpus() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "03:00.0 VGA compatible controller [0300]: NVIDIA Corporation GA104 [GeForce RTX 3070] [10de:2484]\n\tKernel driver in use: nvidia\n04:00.0 VGA compatible controller [0300]: NVIDIA Corporation GA106 [GeForce RTX 3060] [10de:2503]\n\tKernel driver in use: nvidia\n",
        )
        .with_command(
            "glxinfo",
            ["-B"],
            "OpenGL vendor string: NVIDIA Corporation\n\
             OpenGL renderer string: NVIDIA GeForce RTX/PCIe/SSE2\n\
             OpenGL version string: 4.6.0 NVIDIA 535.154.05\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 2);
    for device in &result.devices {
        match &device.properties {
            DeviceProperties::Gpu(gpu) => assert_eq!(gpu.renderer, None),
            other => panic!("expected gpu properties, got {other:?}"),
        }
        assert!(!device
            .sources
            .iter()
            .any(|source| source.source == "glxinfo -B"));
    }
}

#[tokio::test]
async fn gpu_probe_enriches_same_vendor_gpu_when_glxinfo_renderer_matches_unique_model() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "03:00.0 VGA compatible controller [0300]: NVIDIA Corporation GA104 [GeForce RTX 3070] [10de:2484]\n\tKernel driver in use: nvidia\n04:00.0 VGA compatible controller [0300]: NVIDIA Corporation GA106 [GeForce RTX 3060] [10de:2503]\n\tKernel driver in use: nvidia\n",
        )
        .with_command(
            "glxinfo",
            ["-B"],
            "OpenGL vendor string: NVIDIA Corporation\n\
             OpenGL renderer string: NVIDIA GeForce RTX 3060/PCIe/SSE2\n\
             OpenGL version string: 4.6.0 NVIDIA 535.154.05\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 2);
    let rtx_3070 = &result.devices[0];
    let rtx_3060 = &result.devices[1];

    match &rtx_3070.properties {
        DeviceProperties::Gpu(gpu) => assert_eq!(gpu.renderer, None),
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert!(!rtx_3070
        .sources
        .iter()
        .any(|source| source.source == "glxinfo -B"));

    assert_eq!(rtx_3060.name, "NVIDIA GeForce RTX 3060/PCIe/SSE2");
    match &rtx_3060.properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(
                gpu.renderer.as_deref(),
                Some("NVIDIA GeForce RTX 3060/PCIe/SSE2")
            );
            assert_eq!(gpu.opengl_vendor.as_deref(), Some("NVIDIA Corporation"));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert!(rtx_3060
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "glxinfo -B"));
}

#[tokio::test]
async fn gpu_probe_reads_drm_vram_total() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "03:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI] Navi 22 [1002:73df]\n\tKernel driver in use: amdgpu\n",
        )
        .with_glob(
            "/sys/class/drm/*/device/uevent",
            vec![PathBuf::from("/sys/class/drm/card1/device/uevent")],
        )
        .with_file(
            "/sys/class/drm/card1/device/uevent",
            "PCI_SLOT_NAME=0000:03:00.0\n",
        )
        .with_file(
            "/sys/class/drm/card1/device/mem_info_vram_total",
            "8589934592\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    let device = &result.devices[0];
    match &device.properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.memory_bytes, Some(8_589_934_592));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert!(device.sources.iter().any(|source| {
        source.kind == SourceKind::Sysfs
            && source.source == "/sys/class/drm/card1/device/mem_info_vram_total"
    }));
}

#[tokio::test]
async fn gpu_probe_reads_dmesg_vram_total_by_pci_address() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "03:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI] Navi 22 [1002:73df]\n\tKernel driver in use: amdgpu\n",
        )
        .with_command(
            "dmesg",
            std::iter::empty::<&str>(),
            "[    2.123456] [drm] 0000:03:00.0: VRAM: 8192M 0x0000008000000000 - 0x0000009FFFFFFFFF\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    let device = &result.devices[0];
    match &device.properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.memory_bytes, Some(8192 * 1024 * 1024));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Command && source.source == "dmesg"));
}

#[tokio::test]
async fn gpu_probe_reads_nvidia_smi_memory_total_by_pci_address() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "03:00.0 VGA compatible controller [0300]: NVIDIA Corporation GA104 [GeForce RTX 3070] [10de:2484]\n\tKernel driver in use: nvidia\n",
        )
        .with_command(
            "nvidia-smi",
            [
                "--query-gpu=pci.bus_id,memory.total",
                "--format=csv,noheader,nounits",
            ],
            "00000000:03:00.0, 8192\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    let device = &result.devices[0];
    match &device.properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.memory_bytes, Some(8192 * 1024 * 1024));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert!(device.sources.iter().any(|source| {
        source.kind == SourceKind::Command
            && source.source
                == "nvidia-smi --query-gpu=pci.bus_id,memory.total --format=csv,noheader,nounits"
    }));
}

#[tokio::test]
async fn gpu_probe_reads_nvidia_settings_memory_for_unique_nvidia_gpu() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "03:00.0 VGA compatible controller [0300]: NVIDIA Corporation GA104 [GeForce RTX 3070] [10de:2484]\n\tKernel driver in use: nvidia\n",
        )
        .with_command(
            "nvidia-settings",
            ["-q", "VideoRam"],
            "Attribute 'VideoRam' (deepin:0.0): 8388608.\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    let device = &result.devices[0];
    match &device.properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.memory_bytes, Some(8_589_934_592));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert!(device.sources.iter().any(|source| {
        source.kind == SourceKind::Command && source.source == "nvidia-settings -q VideoRam"
    }));
}

#[tokio::test]
async fn gpu_probe_skips_nvidia_settings_memory_for_multiple_nvidia_gpus() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "03:00.0 VGA compatible controller [0300]: NVIDIA Corporation GA104 [GeForce RTX 3070] [10de:2484]\n\tKernel driver in use: nvidia\n04:00.0 VGA compatible controller [0300]: NVIDIA Corporation GA106 [GeForce RTX 3060] [10de:2503]\n\tKernel driver in use: nvidia\n",
        )
        .with_command(
            "nvidia-settings",
            ["-q", "VideoRam"],
            "Attribute 'VideoRam' (deepin:0.0): 8388608.\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 2);
    for device in &result.devices {
        match &device.properties {
            DeviceProperties::Gpu(gpu) => assert_eq!(gpu.memory_bytes, None),
            other => panic!("expected gpu properties, got {other:?}"),
        }
        assert!(!device
            .sources
            .iter()
            .any(|source| source.source == "nvidia-settings -q VideoRam"));
    }
}

#[tokio::test]
async fn gpu_probe_reads_deepin_sysfs_gpu_info_vram_total() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "03:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI] Navi 22 [1002:73df]\n\tKernel driver in use: amdgpu\n",
        )
        .with_file(
            "/sys/bus/pci/devices/0000:03:00.0/gpu-info",
            "VRAM total size: 200000000\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    let device = &result.devices[0];
    match &device.properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.memory_bytes, Some(8_589_934_592));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert!(device.sources.iter().any(|source| {
        source.kind == SourceKind::Sysfs
            && source.source == "/sys/bus/pci/devices/0000:03:00.0/gpu-info"
    }));
}

#[tokio::test]
async fn gpu_probe_reads_jingjia_proc_gpuinfo_memory_size() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "lspci",
            ["-nn", "-k"],
            "03:00.0 VGA compatible controller [0300]: Jingjia Micro JM9 Series Graphics Adapter [0731:7200]\n\tKernel driver in use: jm9\n",
        )
        .with_file("/proc/gpuinfo_0", "Memory Size: 4 GB\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    let device = &result.devices[0];
    match &device.properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.memory_bytes, Some(4 * 1024 * 1024 * 1024));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert!(device
        .sources
        .iter()
        .any(|source| source.kind == SourceKind::Procfs && source.source == "/proc/gpuinfo_0"));
}

#[tokio::test]
async fn gpu_probe_uses_sysfs_display_pci_when_lspci_is_missing() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/bus/pci/devices/*",
            vec![PathBuf::from("/sys/bus/pci/devices/0000:00:02.0")],
        )
        .with_file("/sys/bus/pci/devices/0000:00:02.0/vendor", "0x8086\n")
        .with_file("/sys/bus/pci/devices/0000:00:02.0/device", "0x9a49\n")
        .with_file("/sys/bus/pci/devices/0000:00:02.0/class", "0x030000\n")
        .with_file(
            "/sys/bus/pci/devices/0000:00:02.0/subsystem_vendor",
            "0x1028\n",
        )
        .with_file(
            "/sys/bus/pci/devices/0000:00:02.0/subsystem_device",
            "0x087c\n",
        )
        .with_file("/sys/bus/pci/devices/0000:00:02.0/uevent", "DRIVER=i915\n")
        .with_glob(
            "/sys/bus/pci/devices/0000:00:02.0/driver/module/drivers/*",
            vec![PathBuf::from(
                "/sys/bus/pci/devices/0000:00:02.0/driver/module/drivers/pci:i915",
            )],
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "gpu:pci:0000:00:02.0");
    assert_eq!(result.devices[0].kind, DeviceKind::Gpu);
    assert_eq!(
        result.devices[0].bus,
        Some(BusInfo::Pci {
            address: "0000:00:02.0".to_string(),
            vendor_id: Some("8086".to_string()),
            device_id: Some("9a49".to_string()),
            subsystem_vendor_id: Some("1028".to_string()),
            subsystem_device_id: Some("087c".to_string()),
            class: Some("030000".to_string()),
        })
    );
    match &result.devices[0].properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.vendor.as_deref(), Some("Intel"));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .and_then(|driver| driver.name.as_deref()),
        Some("i915")
    );
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .map(|driver| driver.status),
        Some(DriverStatus::InUse)
    );
    assert_eq!(
        result.devices[0]
            .driver
            .as_ref()
            .map(|driver| driver.modules.as_slice()),
        Some(&["i915".to_string()][..])
    );
    assert_eq!(result.devices[0].sources[0].kind, SourceKind::Sysfs);
    assert_eq!(
        result.consumed[0].id, "pci:0000:00:02.0",
        "sysfs GPU fallback should consume its backing PCI device"
    );
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].source.as_deref(), Some("lspci -nn -k"));
}

#[tokio::test]
async fn gpu_sysfs_fallback_uses_lshw_display_identity() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/bus/pci/devices/*",
            vec![PathBuf::from("/sys/bus/pci/devices/0000:03:00.0")],
        )
        .with_file("/sys/bus/pci/devices/0000:03:00.0/vendor", "0x1234\n")
        .with_file("/sys/bus/pci/devices/0000:03:00.0/device", "0x5678\n")
        .with_file("/sys/bus/pci/devices/0000:03:00.0/class", "0x038000\n")
        .with_command(
            "lshw",
            ["-class", "display"],
            "  *-display\n\
                  product: Jingjia JM9 Series Graphics Adapter\n\
                  vendor: Jingjia Micro\n\
                  bus info: pci@0000:03:00.0\n\
                  configuration: driver=jm9 latency=0\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    let device = &result.devices[0];
    assert_eq!(device.name, "Jingjia JM9 Series Graphics Adapter");
    assert_eq!(device.vendor.as_deref(), Some("Jingjia Micro"));
    match &device.properties {
        DeviceProperties::Gpu(gpu) => {
            assert_eq!(gpu.vendor.as_deref(), Some("Jingjia Micro"));
        }
        other => panic!("expected gpu properties, got {other:?}"),
    }
}

#[tokio::test]
async fn gpu_probe_ignores_non_display_and_non_device_sysfs_pci_entries() {
    let runner = FakeSourceRunner::new()
        .with_glob(
            "/sys/bus/pci/devices/*",
            vec![
                PathBuf::from("/sys/bus/pci/devices/0000:00:1f.3"),
                PathBuf::from("/sys/bus/pci/devices/pci0000:00"),
            ],
        )
        .with_file("/sys/bus/pci/devices/0000:00:1f.3/class", "0x040300\n")
        .with_file("/sys/bus/pci/devices/pci0000:00/class", "0x030000\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));
    let result = GpuProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    assert!(result.consumed.is_empty());
}

#[tokio::test]
async fn monitor_probe_uses_sysfs_edid_when_xrandr_verbose_is_missing() {
    let mut edid = vec![0u8; 128];
    edid[0..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
    edid[8] = 0x05;
    edid[9] = 0xe3;
    edid[16] = 12;
    edid[17] = 32;
    edid[21] = 52;
    edid[22] = 32;
    edid[72] = 0;
    edid[73] = 0;
    edid[74] = 0;
    edid[75] = 0xfc;
    edid[76] = 0;
    edid[77..90].copy_from_slice(b"AOC TEST    \n");
    let checksum = (256u16 - edid[..127].iter().map(|b| *b as u16).sum::<u16>() % 256) % 256;
    edid[127] = checksum as u8;

    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path, edid);
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("HDMI-1"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC"));
            assert_eq!(
                monitor.manufacturer_name.as_deref(),
                Some("AOC International")
            );
            assert_eq!(monitor.product.as_deref(), Some("AOC TEST"));
            assert_eq!(monitor.manufactured_year, Some(2022));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_reports_xrandr_max_resolution() {
    let runner = FakeSourceRunner::new().with_command(
        "xrandr",
        ["--query"],
        "HDMI-1 connected primary 1920x1080+0+0\n\
           2560x1440     59.95 +\n\
           1920x1080     60.00*\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.resolution.as_deref(), Some("1920x1080"));
            assert_eq!(monitor.max_resolution.as_deref(), Some("2560x1440"));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_enriches_single_monitor_from_hwinfo() {
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_command(
            "hwinfo",
            ["--monitor"],
            "31: None 00.0: 10002 LCD Monitor\n\
               Hardware Class: monitor\n\
               Model: \"AOC 24B2W1\"\n\
               Vendor: \"AOC International\"\n\
               Device: eisa 0x1234\n\
               Serial ID: \"MON123\"\n\
               Resolution: 1920x1080@60Hz\n\
               Size: 520x320 mm\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.devices[0]
        .sources
        .iter()
        .any(|source| source.source == "hwinfo --monitor"));
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.product.as_deref(), Some("AOC 24B2W1"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC International"));
            assert_eq!(monitor.serial.as_deref(), Some("MON123"));
            assert_eq!(monitor.size_mm, Some((520, 320)));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_matches_hwinfo_by_unique_resolution() {
    let runner = FakeSourceRunner::new()
        .with_command(
            "xrandr",
            ["--query"],
            "DP-1 connected 2560x1440+1920+0\n\
             HDMI-1 connected 1920x1080+0+0\n",
        )
        .with_command(
            "hwinfo",
            ["--monitor"],
            "31: None 00.0: 10002 LCD Monitor\n\
               Hardware Class: monitor\n\
               Model: \"AOC HDMI\"\n\
               Vendor: \"AOC International\"\n\
               Resolution: 1920x1080@60Hz\n\
\n\
             32: None 00.0: 10002 LCD Monitor\n\
               Hardware Class: monitor\n\
               Model: \"Dell DP\"\n\
               Vendor: \"Dell Inc.\"\n\
               Resolution: 2560x1440@60Hz\n",
        );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 2);
    let by_connector = result
        .devices
        .iter()
        .filter_map(|device| match &device.properties {
            DeviceProperties::Monitor(monitor) => Some((
                monitor.connector.as_deref().unwrap_or_default(),
                monitor.product.as_deref(),
            )),
            _ => None,
        })
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(by_connector.get("DP-1"), Some(&Some("Dell DP")));
    assert_eq!(by_connector.get("HDMI-1"), Some(&Some("AOC HDMI")));
}

#[tokio::test]
async fn monitor_probe_uses_hwinfo_when_xrandr_and_sysfs_are_missing() {
    let runner = FakeSourceRunner::new().with_command(
        "hwinfo",
        ["--monitor"],
        "31: None 00.0: 10002 LCD Monitor\n\
           Hardware Class: monitor\n\
           Model: \"AOC HWINFO\"\n\
           Vendor: \"AOC International\"\n\
           Serial ID: \"HW123\"\n\
           Resolution: 1920x1080@60Hz\n\
           Size: 520x320 mm\n",
    );
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "monitor:hwinfo:HW123");
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(result.warnings[0].source.as_deref(), Some("xrandr --query"));
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.product.as_deref(), Some("AOC HWINFO"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC International"));
            assert_eq!(monitor.serial.as_deref(), Some("HW123"));
            assert_eq!(monitor.resolution.as_deref(), Some("1920x1080"));
            assert_eq!(monitor.size_mm, Some((520, 320)));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_uses_sysfs_edid_when_xrandr_query_is_missing() {
    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path.clone(), monitor_test_edid("AOC SYSFS"));
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "monitor:HDMI-1");
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(result.warnings[0].source.as_deref(), Some("xrandr --query"));
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("HDMI-1"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC"));
            assert_eq!(monitor.product.as_deref(), Some("AOC SYSFS"));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_does_not_create_sysfs_only_device_for_empty_edid() {
    let path = PathBuf::from("/sys/class/drm/card0-DP-1/edid");
    let runner = FakeSourceRunner::new()
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path.clone(), Vec::new());
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert!(result.devices.is_empty());
    let mut codes = result
        .warnings
        .iter()
        .map(|warning| warning.code.as_str())
        .collect::<Vec<_>>();
    codes.sort_unstable();
    assert_eq!(codes, vec!["edid_parse_failed", "source_missing"]);
}

#[tokio::test]
async fn monitor_probe_warns_when_sysfs_edid_read_fails() {
    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()]);
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "source_missing");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some(path.to_str().unwrap())
    );
}

#[tokio::test]
async fn monitor_probe_skips_ambiguous_sysfs_edids_for_duplicate_normalized_connectors() {
    let first_path = PathBuf::from("/sys/class/drm/card0-DP-1/edid");
    let second_path = PathBuf::from("/sys/class/drm/card1-DP-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "DP-1 connected 2560x1440+0+0\n")
        .with_glob(
            "/sys/class/drm/*/edid",
            vec![first_path.clone(), second_path.clone()],
        )
        .with_file_bytes(first_path, monitor_test_edid("AOC FIRST"))
        .with_file_bytes(second_path, monitor_test_edid("AOC SECOND"));
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.warnings.is_empty());
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("DP-1"));
            assert_eq!(monitor.manufacturer, None);
            assert_eq!(monitor.product, None);
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_uses_connected_duplicate_sysfs_edid() {
    let disconnected_path = PathBuf::from("/sys/class/drm/card0-DP-1/edid");
    let connected_path = PathBuf::from("/sys/class/drm/card1-DP-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "DP-1 connected 2560x1440+0+0\n")
        .with_glob(
            "/sys/class/drm/*/edid",
            vec![disconnected_path.clone(), connected_path.clone()],
        )
        .with_file_bytes(disconnected_path.clone(), monitor_test_edid("AOC BAD"))
        .with_file("/sys/class/drm/card0-DP-1/status", "disconnected\n")
        .with_file_bytes(connected_path.clone(), monitor_test_edid("AOC CONN"))
        .with_file("/sys/class/drm/card1-DP-1/status", "connected\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.warnings.is_empty());
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("DP-1"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC"));
            assert_eq!(monitor.product.as_deref(), Some("AOC CONN"));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_uses_enabled_duplicate_sysfs_edid_when_status_is_ambiguous() {
    let disabled_path = PathBuf::from("/sys/class/drm/card0-DP-1/edid");
    let enabled_path = PathBuf::from("/sys/class/drm/card1-DP-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "DP-1 connected 2560x1440+0+0\n")
        .with_glob(
            "/sys/class/drm/*/edid",
            vec![disabled_path.clone(), enabled_path.clone()],
        )
        .with_file_bytes(disabled_path.clone(), monitor_test_edid("AOC DISABLED"))
        .with_file("/sys/class/drm/card0-DP-1/status", "connected\n")
        .with_file("/sys/class/drm/card0-DP-1/enabled", "disabled\n")
        .with_file_bytes(enabled_path.clone(), monitor_test_edid("AOC ENABLED"))
        .with_file("/sys/class/drm/card1-DP-1/status", "connected\n")
        .with_file("/sys/class/drm/card1-DP-1/enabled", "enabled\n");
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.warnings.is_empty());
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("DP-1"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC"));
            assert_eq!(monitor.product.as_deref(), Some("AOC ENABLED"));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_uses_unique_readable_duplicate_sysfs_edid() {
    let readable_path = PathBuf::from("/sys/class/drm/card0-DP-1/edid");
    let unreadable_path = PathBuf::from("/sys/class/drm/card1-DP-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "DP-1 connected 2560x1440+0+0\n")
        .with_glob(
            "/sys/class/drm/*/edid",
            vec![readable_path.clone(), unreadable_path],
        )
        .with_file_bytes(readable_path, monitor_test_edid("AOC READABLE"));
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.warnings.is_empty());
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("DP-1"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC"));
            assert_eq!(monitor.product.as_deref(), Some("AOC READABLE"));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_prefers_xrandr_verbose_edid_over_sysfs_edid() {
    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_command(
            "xrandr",
            ["--verbose"],
            format!(
                "HDMI-1 connected 1920x1080+0+0\n\tEDID:\n{}\n",
                xrandr_edid_hex(&monitor_test_edid("AOC VERBOSE"))
            ),
        )
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path, monitor_test_edid("AOC SYSFS"));
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("HDMI-1"));
            assert_eq!(monitor.product.as_deref(), Some("AOC VERBOSE"));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_reports_edid_gamma_and_diagonal_inches() {
    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path, monitor_test_edid("AOC SYSFS"));
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;
    let properties = format!("{:?}", result.devices[0].properties);

    assert!(properties.contains("gamma: Some(2.2)"));
    assert!(properties.contains("diagonal_inches: Some(24.0)"));
}

#[tokio::test]
async fn monitor_probe_falls_back_to_sysfs_when_xrandr_verbose_edid_is_invalid() {
    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_command(
            "xrandr",
            ["--verbose"],
            "HDMI-1 connected 1920x1080+0+0\n\tEDID:\n\t\t00ff\n",
        )
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path, monitor_test_edid("AOC SYSFS"));
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert!(result.warnings.len() <= 1);
    assert!(result
        .warnings
        .iter()
        .all(|warning| warning.code == "edid_parse_failed"));
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("HDMI-1"));
            assert_eq!(monitor.manufacturer.as_deref(), Some("AOC"));
            assert_eq!(
                monitor.manufacturer_name.as_deref(),
                Some("AOC International")
            );
            assert_eq!(monitor.product.as_deref(), Some("AOC SYSFS"));
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

#[tokio::test]
async fn monitor_probe_reports_edid_parse_warning_and_preserves_device() {
    let path = PathBuf::from("/sys/class/drm/card0-HDMI-A-1/edid");
    let runner = FakeSourceRunner::new()
        .with_command("xrandr", ["--query"], "HDMI-1 connected 1920x1080+0+0\n")
        .with_glob("/sys/class/drm/*/edid", vec![path.clone()])
        .with_file_bytes(path.clone(), vec![0u8; 128]);
    let ctx = ProbeContext::new(&runner, Duration::from_secs(1));

    let result = MonitorProbe.probe(&ctx).await;

    assert_eq!(result.devices.len(), 1);
    assert_eq!(result.devices[0].id, "monitor:HDMI-1");
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].code, "edid_parse_failed");
    assert_eq!(
        result.warnings[0].source.as_deref(),
        Some(path.to_str().unwrap())
    );
    assert_eq!(
        result.warnings[0].device_id.as_deref(),
        Some("monitor:HDMI-1")
    );
    match &result.devices[0].properties {
        DeviceProperties::Monitor(monitor) => {
            assert_eq!(monitor.connector.as_deref(), Some("HDMI-1"));
            assert_eq!(monitor.manufacturer, None);
            assert_eq!(monitor.product, None);
        }
        other => panic!("expected monitor properties, got {other:?}"),
    }
}

fn monitor_test_edid(name: &str) -> Vec<u8> {
    let mut edid = vec![0u8; 128];
    edid[0..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
    edid[8] = 0x05;
    edid[9] = 0xe3;
    edid[16] = 12;
    edid[17] = 32;
    edid[21] = 52;
    edid[22] = 32;
    edid[23] = 120;
    edid[72] = 0;
    edid[73] = 0;
    edid[74] = 0;
    edid[75] = 0xfc;
    edid[76] = 0;
    let mut descriptor = [b' '; 13];
    let name = name.as_bytes();
    let len = name.len().min(12);
    descriptor[..len].copy_from_slice(&name[..len]);
    descriptor[12] = b'\n';
    edid[77..90].copy_from_slice(&descriptor);
    let checksum = (256u16 - edid[..127].iter().map(|b| *b as u16).sum::<u16>() % 256) % 256;
    edid[127] = checksum as u8;
    edid
}

fn xrandr_edid_hex(edid: &[u8]) -> String {
    edid.chunks(16)
        .map(|chunk| {
            let hex = chunk
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            format!("\t\t{hex}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
