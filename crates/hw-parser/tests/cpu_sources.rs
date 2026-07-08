use hw_parser::{
    cpu_extensions_from_flags, infer_cpu_vendor_from_name, merge_cpu_records,
    parse_dmidecode_processor, parse_lscpu, parse_lshw_processor, parse_proc_cpuinfo,
    parse_proc_hardware, DmidecodeCpuRecord,
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
    assert_eq!(cpu.l1d_cache.as_deref(), Some("192 KiB (4 instances)"));
    assert_eq!(cpu.l1i_cache.as_deref(), Some("128 KiB (4 instances)"));
    assert_eq!(cpu.l2_cache.as_deref(), Some("5 MiB (4 instances)"));
    assert_eq!(cpu.l3_cache.as_deref(), Some("12 MiB (1 instance)"));
}

#[test]
fn parse_lscpu_accepts_deepin_proc_style_key_aliases() {
    let cpu = parse_lscpu(
        "Architecture:        x86_64\n\
         CPU(s):              4\n\
         model name:          Intel(R) Core(TM) i3-9100F CPU @ 3.60GHz\n\
         vendor_id:           GenuineIntel\n\
         Thread(s) per core:  1\n\
         bogomips:            7200.00\n\
         cpu family:          6\n\
         CPU MHz:             4085.639\n\
         model:               158\n\
         stepping:            10\n\
         flags:               fpu mmx sse sse2\n\
         Virtualization:      VT-x\n",
    );

    assert_eq!(
        cpu.model_name.as_deref(),
        Some("Intel(R) Core(TM) i3-9100F CPU @ 3.60GHz")
    );
    assert_eq!(cpu.vendor.as_deref(), Some("GenuineIntel"));
    assert_eq!(cpu.threads, Some(4));
    assert_eq!(cpu.threads_per_core, Some(1));
    assert_eq!(cpu.cpu_family.as_deref(), Some("6"));
    assert_eq!(cpu.cpu_model.as_deref(), Some("158"));
    assert_eq!(cpu.stepping.as_deref(), Some("10"));
    assert_eq!(cpu.bogomips.as_deref(), Some("7200.00"));
    assert_eq!(cpu.cpu_mhz, Some(4086));
    assert_eq!(cpu.flags, vec!["fpu", "mmx", "sse", "sse2"]);
    assert_eq!(cpu.virtualization.as_deref(), Some("VT-x"));
}

#[test]
fn parse_lscpu_totals_old_shared_cache_fields() {
    let cpu = parse_lscpu(
        "Architecture:            x86_64\n\
         CPU(s):                  8\n\
         Model name:              Intel(R) Core(TM) i7-1185G7\n\
         L1d cache:               32K\n\
         L1d shared cpu list:     0,4\n\
         L1i cache:               32K\n\
         L1i shared cpu list:     0,4\n\
         L2 cache:                256K\n\
         L2 shared cpu list:      0,4\n\
         L3 cache:                8M\n\
         L3 shared cpu list:      0-7\n\
         L4 cache:                16M\n\
         L4 shared cpu list:      0-7\n",
    );

    assert_eq!(cpu.l1d_cache.as_deref(), Some("128 KiB"));
    assert_eq!(cpu.l1i_cache.as_deref(), Some("128 KiB"));
    assert_eq!(cpu.l2_cache.as_deref(), Some("1 MiB"));
    assert_eq!(cpu.l3_cache.as_deref(), Some("8 MiB"));
    assert_eq!(cpu.l4_cache.as_deref(), Some("16 MiB"));
}

#[test]
fn parse_lscpu_infers_cores_from_threads_per_core_when_core_field_is_missing() {
    let cpu = parse_lscpu(
        "Architecture:            x86_64\n\
         CPU(s):                  8\n\
         Thread(s) per core:      2\n\
         Socket(s):               1\n\
         Model name:              Intel(R) Core(TM) i7-1185G7\n",
    );

    assert_eq!(cpu.threads, Some(8));
    assert_eq!(cpu.sockets, Some(1));
    assert_eq!(cpu.cores_per_socket, Some(4));
    let json = serde_json::to_value(cpu).unwrap();
    assert_eq!(json["threads_per_core"], 2);
}

#[test]
fn parse_lscpu_reconciles_inconsistent_core_topology() {
    let cpu = parse_lscpu(
        "Architecture:            x86_64\n\
         CPU(s):                  6\n\
         Core(s) per socket:      4\n\
         Thread(s) per core:      2\n\
         Socket(s):               1\n\
         Model name:              ZHAOXIN KaiXian KX-U6780A@2.7GHz\n",
    );

    assert_eq!(cpu.threads, Some(6));
    assert_eq!(cpu.sockets, Some(1));
    assert_eq!(cpu.threads_per_core, Some(2));
    assert_eq!(cpu.cores_per_socket, Some(3));
}

#[test]
fn parse_lscpu_reads_online_cpu_list() {
    let cpu = parse_lscpu(
        "Architecture:            x86_64\n\
         CPU(s):                  8\n\
         On-line CPU(s) list:     0-3,6\n\
         Core(s) per socket:      4\n\
         Thread(s) per core:      2\n\
         Socket(s):               1\n\
         Model name:              Intel(R) Core(TM) i7-1185G7\n",
    );

    assert_eq!(cpu.threads, Some(8));
    assert_eq!(cpu.online_threads, Some(5));
    assert_eq!(cpu.online_cores, Some(3));
}

#[test]
fn cpu_extensions_follow_deepin_display_order() {
    let flags = ["fpu", "sse4_2", "sse2", "sse", "ssse3", "mmx"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

    assert_eq!(
        cpu_extensions_from_flags(&flags),
        vec!["MMX", "SSE", "SSE2", "SSE4", "SSSE3", "SSE4_2"]
    );
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
    let json = serde_json::to_value(&cpu).unwrap();
    assert_eq!(json["cpu_implementer"], "0x70");
    assert_eq!(json["cpu_architecture"], "8");
    assert_eq!(json["cpu_variant"], "0x1");
    assert_eq!(json["cpu_part"], "0x660");
    assert_eq!(json["cpu_revision"], "2");

    let merged = merge_cpu_records(Some(cpu), None, &[]);
    let json = serde_json::to_value(merged).unwrap();
    assert_eq!(json["cpu_implementer"], "0x70");
    assert_eq!(json["cpu_architecture"], "8");
    assert_eq!(json["cpu_variant"], "0x1");
    assert_eq!(json["cpu_part"], "0x660");
    assert_eq!(json["cpu_revision"], "2");
}

#[test]
fn parse_proc_cpuinfo_reads_cache_size_as_l2_cache() {
    let cpu = parse_proc_cpuinfo(
        "processor\t: 0\n\
         model name\t: Intel(R) Core(TM) i7-1185G7 @ 3.00GHz\n\
         cache size\t: 12288 KB\n",
    );

    assert_eq!(cpu.l2_cache.as_deref(), Some("12288 KB"));
}

#[test]
fn parse_proc_cpuinfo_reads_clflush_size() {
    let cpu = parse_proc_cpuinfo(
        "processor\t: 0\n\
         model name\t: Intel(R) Core(TM) i7-1185G7 @ 3.00GHz\n\
         clflush size\t: 64\n",
    );

    assert_eq!(serde_json::to_value(cpu).unwrap()["clflush_size_bytes"], 64);
}

#[test]
fn parse_proc_cpuinfo_reads_uppercase_cpu_mhz() {
    let cpu = parse_proc_cpuinfo(
        "processor\t: 0\n\
         model name\t: Loongson-3A5000\n\
         CPU MHz\t\t: 2500.000\n",
    );

    assert_eq!(cpu.cpu_mhz, Some(2500));
}

#[test]
fn parse_proc_cpuinfo_reads_deepin_lowercase_frequency_and_features() {
    let cpu = parse_proc_cpuinfo(
        "processor\t: 0\n\
         model name\t: HUAWEI Kunpeng 920\n\
         cpu mhz\t\t: 2600.000\n\
         features\t: fp asimd evtstrm crc32\n",
    );

    assert_eq!(cpu.cpu_mhz, Some(2600));
    assert_eq!(cpu.flags, vec!["fp", "asimd", "evtstrm", "crc32"]);
}

#[test]
fn parse_proc_cpuinfo_reads_x86_topology_fields() {
    let cpu = parse_proc_cpuinfo(
        "processor\t: 0\n\
         physical id\t: 0\n\
         siblings\t: 4\n\
         cpu cores\t: 2\n\
         \n\
         processor\t: 1\n\
         physical id\t: 0\n\
         siblings\t: 4\n\
         cpu cores\t: 2\n\
         \n\
         processor\t: 2\n\
         physical id\t: 0\n\
         siblings\t: 4\n\
         cpu cores\t: 2\n\
         \n\
         processor\t: 3\n\
         physical id\t: 0\n\
         siblings\t: 4\n\
         cpu cores\t: 2\n",
    );

    assert_eq!(cpu.threads, Some(4));
    assert_eq!(cpu.sockets, Some(1));
    assert_eq!(cpu.cores_per_socket, Some(2));
    assert_eq!(cpu.threads_per_core, Some(2));
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
fn merge_cpu_records_strips_deepin_instance_count_suffix() {
    let merged = merge_cpu_records(
        Some(hw_parser::CpuRecord {
            model_name: Some("Intel(R) Core(TM) i7-1185G7 x 8".to_string()),
            ..Default::default()
        }),
        None,
        &[],
    );

    assert_eq!(merged.name.as_deref(), Some("Intel(R) Core(TM) i7-1185G7"));
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
fn parse_dmidecode_reads_kylin_cpu_slot_serial_and_external_clock() {
    let records = parse_dmidecode_processor(
        "Handle 0x0041, DMI type 4, 48 bytes\n\
         Processor Information\n\
                 Socket Designation: CPU 0\n\
                 Manufacturer: GenuineIntel\n\
                 Version: Intel(R) Core(TM) i7-1185G7\n\
                 Serial Number: CPU-SERIAL-1\n\
                 External Clock: 100 MHz\n",
    );

    assert_eq!(
        serde_json::to_value(&records[0]).unwrap()["serial_number"],
        "CPU-SERIAL-1"
    );
    assert_eq!(
        serde_json::to_value(&records[0]).unwrap()["external_clock_mhz"],
        100
    );
}

#[test]
fn parse_dmidecode_reads_core_enabled_and_merge_exposes_enabled_cores() {
    let records = parse_dmidecode_processor(
        "Handle 0x0041, DMI type 4, 48 bytes\n\
         Processor Information\n\
                 Socket Designation: CPU 0\n\
                 Manufacturer: GenuineIntel\n\
                 Version: Intel(R) Xeon(R)\n\
                 Core Count: 8\n\
                 Core Enabled: 6\n\
                 Thread Count: 12\n\
         Handle 0x0042, DMI type 4, 48 bytes\n\
         Processor Information\n\
                 Socket Designation: CPU 1\n\
                 Manufacturer: GenuineIntel\n\
                 Version: Intel(R) Xeon(R)\n\
                 Core Count: 8\n\
                 Core Enabled: 5\n\
                 Thread Count: 10\n",
    );

    assert_eq!(records[0].core_enabled, Some(6));
    assert_eq!(records[1].core_enabled, Some(5));

    let merged = merge_cpu_records(None, None, &records);
    assert_eq!(merged.enabled_cores, Some(11));
}

#[test]
fn merge_cpu_records_uses_dmi_family_even_when_it_is_not_device_evidence() {
    let merged = merge_cpu_records(
        Some(hw_parser::CpuRecord {
            architecture: Some("x86_64".to_string()),
            threads: Some(8),
            ..Default::default()
        }),
        None,
        &[DmidecodeCpuRecord {
            family: Some("Server".to_string()),
            ..Default::default()
        }],
    );

    assert_eq!(merged.family.as_deref(), Some("Server"));
    assert_eq!(merged.threads, Some(8));
}

#[test]
fn parse_dmidecode_ignores_unknown_identity_values() {
    let records = parse_dmidecode_processor(
        "Handle 0x0041, DMI type 4, 48 bytes\n\
         Processor Information\n\
                 Socket Designation: CPU 0\n\
                 Manufacturer: Unknown\n\
                 Version: Unknown\n\
                 Family: Unknown\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].manufacturer, None);
    assert_eq!(records[0].version, None);
    assert_eq!(records[0].family, None);
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
