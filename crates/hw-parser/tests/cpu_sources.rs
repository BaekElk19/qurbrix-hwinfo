use hw_parser::{
    infer_cpu_vendor_from_name, merge_cpu_records, parse_dmidecode_processor, parse_lscpu,
    parse_lshw_processor, parse_proc_cpuinfo, parse_proc_hardware, DmidecodeCpuRecord,
};
use hw_testdata::fixture;

#[test]
fn parse_lscpu_reads_extended_cpu_fields() {
    let cpu = parse_lscpu(&fixture("cpu/lscpu-intel-x86_64.txt"));

    assert_eq!(cpu.architecture.as_deref(), Some("x86_64"));
    assert_eq!(cpu.vendor.as_deref(), Some("GenuineIntel"));
    assert_eq!(
        cpu.model_name.as_deref(),
        Some("Intel(R) Core(TM) i7-1185G7")
    );
    assert_eq!(cpu.threads, Some(16));
    assert_eq!(cpu.cores_per_socket, Some(4));
    assert_eq!(cpu.sockets, Some(1));
    assert_eq!(cpu.cpu_mhz, Some(1800));
    assert_eq!(cpu.cpu_max_mhz, Some(4800));
    assert_eq!(cpu.cpu_min_mhz, Some(400));
    assert_eq!(cpu.cpu_family.as_deref(), Some("6"));
    assert_eq!(cpu.cpu_model.as_deref(), Some("140"));
    assert_eq!(cpu.stepping.as_deref(), Some("1"));
    assert!(cpu.flags.contains(&"fpu".to_string()));
    assert_eq!(cpu.virtualization.as_deref(), Some("VT-x"));
}

#[test]
fn parse_proc_cpuinfo_uses_hardware_and_processor_fallbacks() {
    let cpu = parse_proc_cpuinfo(
        "processor\t: 0\n\
         BogoMIPS\t: 100.00\n\
         Features\t: fp asimd evtstrm crc32\n\
         CPU implementer\t: 0x70\n\
         CPU architecture: 8\n\
         CPU variant\t: 0x1\n\
         CPU part\t: 0x660\n\
         CPU revision\t: 2\n\
         cpu MHz\t\t: 2300.000\n\
         \n\
         processor\t: 1\n\
         BogoMIPS\t: 100.00\n\
         Features\t: fp asimd evtstrm crc32\n\
         CPU implementer\t: 0x70\n\
         CPU architecture: 8\n\
         CPU variant\t: 0x1\n\
         CPU part\t: 0x660\n\
         CPU revision\t: 2\n\
         cpu MHz\t\t: 2300.000\n\
         \n\
         Hardware\t: Phytium D2000/8\n\
         Processor\t: AArch64 Processor rev 2 (aarch64)\n",
    );

    assert_eq!(cpu.model_name.as_deref(), Some("Phytium D2000/8"));
    assert_eq!(cpu.architecture.as_deref(), Some("aarch64"));
    assert_eq!(cpu.threads, Some(2));
    assert_eq!(cpu.cpu_mhz, Some(2300));
    assert_eq!(cpu.bogomips.as_deref(), Some("100.00"));
    assert_eq!(cpu.flags, vec!["fp", "asimd", "evtstrm", "crc32"]);
}

#[test]
fn parse_proc_cpuinfo_reads_loongarch_cpu_model() {
    let cpu = parse_proc_cpuinfo(&fixture("cpu/proc-cpuinfo-loongarch.txt"));

    assert_eq!(cpu.model_name.as_deref(), Some("Loongson-3A5000"));
    assert_eq!(cpu.threads, Some(2));
    assert_eq!(cpu.bogomips.as_deref(), Some("4800.00"));
    assert_eq!(cpu.flags, vec!["cpucfg", "lam", "ual", "fpu"]);
}

#[test]
fn parse_proc_cpuinfo_covers_domestic_and_x86_vendor_samples() {
    for (path, expected_name, expected_vendor, expected_threads, expected_architecture) in [
        (
            "cpu/proc-cpuinfo-intel-x86_64.txt",
            "Intel(R) Core(TM) i7-1185G7 @ 3.00GHz",
            "Intel",
            2,
            None,
        ),
        (
            "cpu/proc-cpuinfo-amd-x86_64.txt",
            "AMD Ryzen 7 PRO 7840U w/ Radeon 780M Graphics",
            "AMD",
            2,
            None,
        ),
        (
            "cpu/proc-cpuinfo-hygon.txt",
            "Hygon C86 7285 32-core Processor",
            "Hygon",
            2,
            None,
        ),
        (
            "cpu/proc-cpuinfo-zhaoxin.txt",
            "ZHAOXIN KaiXian KX-U6780A@2.7GHz",
            "Zhaoxin",
            2,
            None,
        ),
        (
            "cpu/proc-cpuinfo-phytium-arm64.txt",
            "Phytium D2000/8",
            "Phytium",
            2,
            Some("aarch64"),
        ),
        (
            "cpu/proc-cpuinfo-kunpeng-arm64.txt",
            "Kunpeng 920",
            "HiSilicon",
            2,
            Some("aarch64"),
        ),
        (
            "cpu/proc-cpuinfo-hisilicon-kirin.txt",
            "HUAWEI Kirin 9006C",
            "HiSilicon",
            2,
            Some("aarch64"),
        ),
        (
            "cpu/proc-cpuinfo-sunway.txt",
            "Sunway SW1621",
            "Sunway",
            2,
            Some("sw_64"),
        ),
    ] {
        let cpu = parse_proc_cpuinfo(&fixture(path));

        assert_eq!(cpu.model_name.as_deref(), Some(expected_name), "{path}");
        assert_eq!(cpu.threads, Some(expected_threads), "{path}");
        assert_eq!(cpu.architecture.as_deref(), expected_architecture, "{path}");
        assert_eq!(
            cpu.model_name
                .as_deref()
                .and_then(infer_cpu_vendor_from_name),
            Some(expected_vendor),
            "{path}"
        );
    }
}

#[test]
fn parse_proc_hardware_recognizes_kirin_soc_names() {
    for (input, expected) in [
        ("Hardware\t: HUAWEI Kirin 990\n", "HUAWEI Kirin 990"),
        ("Hardware\t: kirin990\n", "HUAWEI Kirin 990"),
        ("Hardware\t: HUAWEI Kirin 9006C\n", "HUAWEI Kirin 9006C"),
    ] {
        let cpu = parse_proc_hardware(input);

        assert_eq!(cpu.model_name.as_deref(), Some(expected));
    }
}

#[test]
fn parse_lshw_falls_back_from_null_product_to_version() {
    let lshw = parse_lshw_processor(&fixture("cpu/lshw-product-null.txt"));
    let merged = merge_cpu_records(None, Some(lshw), &[]);

    assert_eq!(merged.name.as_deref(), Some("Phytium D2000/8"));
    assert_eq!(merged.vendor.as_deref(), Some("Phytium"));
}

#[test]
fn parse_dmidecode_reads_multiple_processor_sockets() {
    let records = parse_dmidecode_processor(&fixture("cpu/dmidecode-4-dual-socket.txt"));

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].socket_designation.as_deref(), Some("CPU 0"));
    assert_eq!(records[0].manufacturer.as_deref(), Some("HiSilicon"));
    assert_eq!(records[0].version.as_deref(), Some("Kunpeng 920"));
    assert_eq!(records[0].max_speed_mhz, Some(2600));
    assert_eq!(records[0].current_speed_mhz, Some(2400));
    assert_eq!(records[0].core_count, Some(48));
    assert_eq!(records[0].thread_count, Some(48));
    assert_eq!(records[1].socket_designation.as_deref(), Some("CPU 1"));
    assert_eq!(records[1].manufacturer.as_deref(), Some("HiSilicon"));
    assert_eq!(records[1].version.as_deref(), Some("Kunpeng 920"));
    assert_eq!(records[1].max_speed_mhz, Some(2600));
    assert_eq!(records[1].current_speed_mhz, Some(2400));
    assert_eq!(records[1].core_count, Some(48));
    assert_eq!(records[1].thread_count, Some(48));
}

#[test]
fn merge_cpu_records_protects_loongson_name_and_uses_dmi_counts() {
    let lscpu = parse_lscpu(&fixture("cpu/lscpu-loongson-loongarch64.txt"));
    let dmi = vec![
        DmidecodeCpuRecord {
            socket_designation: Some("CPU 0".to_string()),
            manufacturer: Some("Loongson".to_string()),
            version: Some("Loongson-3A5000".to_string()),
            current_speed_mhz: Some(2400),
            core_count: Some(48),
            thread_count: Some(48),
            ..Default::default()
        },
        DmidecodeCpuRecord {
            socket_designation: Some("CPU 1".to_string()),
            manufacturer: Some("Loongson".to_string()),
            version: Some("Loongson-3A5000".to_string()),
            current_speed_mhz: Some(2400),
            core_count: Some(48),
            thread_count: Some(48),
            ..Default::default()
        },
    ];

    let merged = merge_cpu_records(Some(lscpu), None, &dmi);

    assert_eq!(merged.name.as_deref(), Some("Loongson-3A5000"));
    assert_eq!(merged.sockets, Some(2));
    assert_eq!(merged.cores, Some(96));
    assert_eq!(merged.threads, Some(96));
    assert_eq!(merged.current_freq_mhz, Some(2400));
}
