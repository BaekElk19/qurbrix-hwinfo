use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CpuRecord {
    pub architecture: Option<String>,
    pub threads: Option<u32>,
    pub model_name: Option<String>,
    pub vendor: Option<String>,
    pub cores_per_socket: Option<u32>,
    pub sockets: Option<u32>,
    pub cpu_mhz: Option<u32>,
    pub cpu_max_mhz: Option<u32>,
    pub cpu_min_mhz: Option<u32>,
    pub cpu_family: Option<String>,
    pub cpu_model: Option<String>,
    pub stepping: Option<String>,
    pub bogomips: Option<String>,
    pub flags: Vec<String>,
    pub virtualization: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LshwCpuRecord {
    pub product: Option<String>,
    pub vendor: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DmidecodeCpuRecord {
    pub socket_designation: Option<String>,
    pub manufacturer: Option<String>,
    pub version: Option<String>,
    pub family: Option<String>,
    pub max_speed_mhz: Option<u32>,
    pub current_speed_mhz: Option<u32>,
    pub core_count: Option<u32>,
    pub thread_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MergedCpu {
    pub architecture: Option<String>,
    pub threads: Option<u32>,
    pub name: Option<String>,
    pub vendor: Option<String>,
    pub cores: Option<u32>,
    pub sockets: Option<u32>,
    pub max_freq_mhz: Option<u32>,
    pub min_freq_mhz: Option<u32>,
    pub current_freq_mhz: Option<u32>,
    pub family: Option<String>,
    pub model: Option<String>,
    pub stepping: Option<String>,
    pub bogomips: Option<String>,
    pub virtualization: Option<String>,
    pub flags: Vec<String>,
}

pub fn parse_lscpu(input: &str) -> CpuRecord {
    let mut record = CpuRecord::default();
    for line in input.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Architecture" => record.architecture = clean_value(value),
            "CPU(s)" => record.threads = clean_value(value).and_then(|value| value.parse().ok()),
            "Model name" => record.model_name = clean_value(value),
            "Vendor ID" => record.vendor = clean_value(value),
            "Core(s) per socket" => {
                record.cores_per_socket = clean_value(value).and_then(|value| value.parse().ok())
            }
            "Socket(s)" => record.sockets = clean_value(value).and_then(|value| value.parse().ok()),
            "CPU MHz" => record.cpu_mhz = parse_mhz(value),
            "CPU max MHz" => record.cpu_max_mhz = parse_mhz(value),
            "CPU min MHz" => record.cpu_min_mhz = parse_mhz(value),
            "CPU family" => record.cpu_family = clean_value(value),
            "Model" => record.cpu_model = clean_value(value),
            "Stepping" => record.stepping = clean_value(value),
            "BogoMIPS" => record.bogomips = clean_value(value),
            "Flags" => record.flags = value.split_whitespace().map(str::to_string).collect(),
            "Virtualization" => record.virtualization = clean_value(value),
            _ => {}
        }
    }
    record
}

pub fn parse_proc_cpuinfo(input: &str) -> CpuRecord {
    let mut record = CpuRecord::default();
    let mut processor_count = 0u32;

    for line in input.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "model name" | "cpu model" => {
                assign_if_empty(&mut record.model_name, clean_value(value))
            }
            "Hardware" => {
                if let Some(value) = clean_value(value) {
                    if should_use_hardware_model(record.model_name.as_deref()) {
                        record.model_name = Some(value);
                    }
                }
            }
            "Processor" => {
                assign_if_empty(&mut record.model_name, clean_value(value));
                assign_if_empty(&mut record.architecture, architecture_from_processor(value));
            }
            "vendor_id" => assign_if_empty(&mut record.vendor, clean_value(value)),
            "processor" => {
                if clean_value(value)
                    .and_then(|value| value.parse::<u32>().ok())
                    .is_some()
                {
                    processor_count = processor_count.saturating_add(1);
                }
            }
            "cpu MHz" => {
                if record.cpu_mhz.is_none() {
                    record.cpu_mhz = parse_mhz(value);
                }
            }
            "BogoMIPS" | "bogomips" => assign_if_empty(&mut record.bogomips, clean_value(value)),
            "flags" | "Features" => {
                if record.flags.is_empty() {
                    record.flags = value.split_whitespace().map(str::to_string).collect();
                }
            }
            "CPU architecture" | "Architecture" => {
                assign_if_empty(&mut record.architecture, proc_architecture(value));
            }
            _ => {}
        }
    }

    if processor_count > 0 {
        record.threads = Some(processor_count);
    }

    record
}

pub fn parse_proc_hardware(input: &str) -> CpuRecord {
    let mut record = CpuRecord::default();
    let input_lc = input.to_ascii_lowercase();

    if input.contains("HUAWEI Kirin 9006C") {
        record.model_name = Some("HUAWEI Kirin 9006C".to_string());
    } else if input.contains("HUAWEI Kirin 990") || input_lc.contains("kirin990") {
        record.model_name = Some("HUAWEI Kirin 990".to_string());
    }

    record
}

pub fn parse_lshw_processor(input: &str) -> LshwCpuRecord {
    let mut record = LshwCpuRecord::default();
    for line in input.lines() {
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let value = clean_value(value);
        match key.trim() {
            "product" => record.product = value,
            "vendor" => record.vendor = value,
            "version" => record.version = value,
            _ => {}
        }
    }
    record
}

pub fn parse_dmidecode_processor(input: &str) -> Vec<DmidecodeCpuRecord> {
    let mut records = Vec::new();
    let mut current: Option<DmidecodeCpuRecord> = None;

    for line in input.lines() {
        if line.contains("DMI type 4") {
            if let Some(record) = current.take() {
                records.push(record);
            }
            current = Some(DmidecodeCpuRecord::default());
            continue;
        }

        let Some(record) = current.as_mut() else {
            continue;
        };
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Socket Designation" => record.socket_designation = clean_value(value),
            "Manufacturer" => record.manufacturer = clean_value(value),
            "Version" => record.version = clean_value(value),
            "Family" => record.family = clean_value(value),
            "Max Speed" => record.max_speed_mhz = parse_mhz(value),
            "Current Speed" => record.current_speed_mhz = parse_mhz(value),
            "Core Count" => record.core_count = value.parse().ok(),
            "Thread Count" => record.thread_count = value.parse().ok(),
            _ => {}
        }
    }

    if let Some(record) = current {
        records.push(record);
    }
    records
}

pub fn merge_cpu_records(
    lscpu: Option<CpuRecord>,
    lshw: Option<LshwCpuRecord>,
    dmi: &[DmidecodeCpuRecord],
) -> MergedCpu {
    let useful_dmi: Vec<_> = dmi.iter().filter(|record| record.is_useful()).collect();
    let dmi_version = useful_dmi.iter().find_map(|record| record.version.clone());
    let dmi_manufacturer = useful_dmi
        .iter()
        .find_map(|record| record.manufacturer.clone());
    let dmi_max_speed_mhz = useful_dmi.iter().find_map(|record| record.max_speed_mhz);
    let dmi_current_speed_mhz = useful_dmi
        .iter()
        .find_map(|record| record.current_speed_mhz);
    let mut name = lscpu.as_ref().and_then(|record| record.model_name.clone());
    if !current_name_contains_loongson(name.as_deref()) {
        if let Some(candidate) = lshw.as_ref().and_then(lshw_name_candidate) {
            name = Some(candidate);
        }
    }
    if !current_name_contains_loongson(name.as_deref()) {
        if let Some(candidate) = dmi_version {
            name = Some(candidate);
        }
    }
    if name.as_ref().is_none_or(|value| value.trim().is_empty()) {
        name = Some("CPU".to_string());
    }

    let mut vendor = lscpu.as_ref().and_then(|record| record.vendor.clone());
    if let Some(candidate) = lshw.as_ref().and_then(|record| record.vendor.clone()) {
        vendor = Some(candidate);
    }
    if let Some(candidate) = dmi_manufacturer {
        vendor = Some(candidate);
    }

    let lscpu_cores = lscpu
        .as_ref()
        .and_then(|record| record.cores_per_socket?.checked_mul(record.sockets?));
    let lscpu_threads = lscpu.as_ref().and_then(|record| record.threads);
    let sockets = dmi_unique_sockets(&useful_dmi)
        .or_else(|| lscpu.as_ref().and_then(|record| record.sockets))
        .or_else(|| (!useful_dmi.is_empty()).then_some(useful_dmi.len() as u32));
    let (cores, threads) = merge_cpu_counts(lscpu_cores, lscpu_threads, &useful_dmi);

    MergedCpu {
        architecture: lscpu
            .as_ref()
            .and_then(|record| record.architecture.clone()),
        threads,
        name,
        vendor,
        cores,
        sockets,
        max_freq_mhz: lscpu
            .as_ref()
            .and_then(|record| record.cpu_max_mhz)
            .or(dmi_max_speed_mhz),
        min_freq_mhz: lscpu.as_ref().and_then(|record| record.cpu_min_mhz),
        current_freq_mhz: dmi_current_speed_mhz
            .or_else(|| lscpu.as_ref().and_then(|record| record.cpu_mhz)),
        family: lscpu.as_ref().and_then(|record| record.cpu_family.clone()),
        model: lscpu.as_ref().and_then(|record| record.cpu_model.clone()),
        stepping: lscpu.as_ref().and_then(|record| record.stepping.clone()),
        bogomips: lscpu.as_ref().and_then(|record| record.bogomips.clone()),
        virtualization: lscpu
            .as_ref()
            .and_then(|record| record.virtualization.clone()),
        flags: lscpu
            .as_ref()
            .map(|record| record.flags.clone())
            .unwrap_or_default(),
    }
}

impl CpuRecord {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

impl LshwCpuRecord {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

impl DmidecodeCpuRecord {
    pub fn is_useful(&self) -> bool {
        self.manufacturer.is_some()
            || self.version.is_some()
            || self.max_speed_mhz.is_some_and(|value| value > 0)
            || self.current_speed_mhz.is_some_and(|value| value > 0)
            || self.core_count.is_some_and(|value| value > 0)
            || self.thread_count.is_some_and(|value| value > 0)
    }
}

fn merge_cpu_counts(
    lscpu_cores: Option<u32>,
    lscpu_threads: Option<u32>,
    dmi: &[&DmidecodeCpuRecord],
) -> (Option<u32>, Option<u32>) {
    let dmi_cores = sum_dmi_count(dmi.iter().filter_map(|record| record.core_count))
        .filter(|value| *value <= 512);
    let dmi_threads = sum_dmi_count(dmi.iter().filter_map(|record| record.thread_count))
        .filter(|value| *value < 1024);
    let mut cores = lscpu_cores.or(dmi_cores);
    let mut threads = lscpu_threads.or(dmi_threads);

    if let (Some(dmi_cores), Some(current_cores)) = (dmi_cores, cores) {
        if dmi_cores > current_cores {
            cores = Some(dmi_cores);
        }
    }
    if let (Some(dmi_threads), Some(current_threads)) = (dmi_threads, threads) {
        if dmi_threads > current_threads {
            threads = Some(dmi_threads);
        }
    }
    (cores, threads)
}

fn dmi_unique_sockets(dmi: &[&DmidecodeCpuRecord]) -> Option<u32> {
    let mut sockets = Vec::new();
    for socket in dmi
        .iter()
        .filter_map(|record| record.socket_designation.as_deref())
        .filter(|socket| !socket.trim().is_empty())
    {
        if !sockets.contains(&socket) {
            sockets.push(socket);
        }
    }
    (!sockets.is_empty()).then_some(sockets.len() as u32)
}

fn sum_dmi_count(values: impl Iterator<Item = u32>) -> Option<u32> {
    let mut total: u32 = 0;
    let mut seen = false;
    for value in values {
        total = total.checked_add(value)?;
        seen = true;
    }
    seen.then_some(total)
}

fn lshw_name_candidate(record: &LshwCpuRecord) -> Option<String> {
    let product = record.product.as_deref().unwrap_or_default();
    let product_lc = product.to_ascii_lowercase();
    if product.trim().is_empty() || product_lc.contains("null") || product_lc.contains("armv") {
        record
            .version
            .clone()
            .filter(|value| !value.trim().is_empty())
    } else {
        Some(product.to_string())
    }
}

fn current_name_contains_loongson(value: Option<&str>) -> bool {
    value
        .map(|value| value.to_ascii_lowercase().contains("loongson"))
        .unwrap_or(false)
}

fn parse_mhz(value: &str) -> Option<u32> {
    value
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<f32>().ok())
        .map(|value| value.round() as u32)
}

fn assign_if_empty(target: &mut Option<String>, value: Option<String>) {
    if target.is_none() {
        *target = value;
    }
}

fn proc_architecture(value: &str) -> Option<String> {
    let value = clean_value(value)?;
    match value.as_str() {
        "8" => Some("aarch64".to_string()),
        _ => Some(value),
    }
}

fn architecture_from_processor(value: &str) -> Option<String> {
    let value_lc = value.to_ascii_lowercase();
    if value_lc.contains("aarch64") {
        Some("aarch64".to_string())
    } else if value_lc.contains("x86_64") {
        Some("x86_64".to_string())
    } else {
        None
    }
}

fn should_use_hardware_model(current: Option<&str>) -> bool {
    current.is_none_or(|value| {
        let value = value.trim().to_ascii_lowercase();
        value.is_empty() || value.contains(" processor rev ")
    })
}

fn clean_value(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && !value.eq_ignore_ascii_case("Not Specified")).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_dmi_counts_and_keeps_loongson_lscpu_name() {
        let lscpu = CpuRecord {
            architecture: Some("loongarch64".to_string()),
            threads: Some(32),
            model_name: Some("Loongson-3A5000".to_string()),
            vendor: Some("Loongson".to_string()),
            cores_per_socket: Some(16),
            sockets: Some(1),
            ..Default::default()
        };
        let lshw = LshwCpuRecord {
            product: Some("Loongson 3A6000".to_string()),
            vendor: Some("Loongson Technology".to_string()),
            version: None,
        };
        let dmi = vec![DmidecodeCpuRecord {
            socket_designation: Some("CPU 0".to_string()),
            manufacturer: Some("Loongson".to_string()),
            version: Some("Generic DMI CPU".to_string()),
            core_count: Some(64),
            thread_count: Some(128),
            ..Default::default()
        }];

        let merged = merge_cpu_records(Some(lscpu), Some(lshw), &dmi);

        assert_eq!(merged.name.as_deref(), Some("Loongson-3A5000"));
        assert_eq!(merged.cores, Some(64));
        assert_eq!(merged.threads, Some(128));
    }

    #[test]
    fn parses_dmidecode_processor_records() {
        let records = parse_dmidecode_processor(
            "Handle 0x0041, DMI type 4, 48 bytes\n\
             Processor Information\n\
             \tSocket Designation: CPU 0\n\
             \tManufacturer: HiSilicon\n\
             \tVersion: Kunpeng 920\n\
             \tFamily: ARMv8\n\
             \tMax Speed: 2600 MHz\n\
             \tCurrent Speed: 2400 MHz\n\
             \tCore Count: 48\n\
             \tThread Count: 48\n",
        );

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].version.as_deref(), Some("Kunpeng 920"));
        assert_eq!(records[0].current_speed_mhz, Some(2400));
        assert_eq!(records[0].core_count, Some(48));
    }

    #[test]
    fn parse_lscpu_cleans_empty_and_not_specified_values() {
        let record = parse_lscpu(
            "Architecture: aarch64\n\
             Model name: Not Specified\n\
             Vendor ID: \n\
             CPU family: Not Specified\n\
             CPU MHz: 2400.4 MHz\n\
             Virtualization: Not Specified\n",
        );

        assert_eq!(record.architecture.as_deref(), Some("aarch64"));
        assert_eq!(record.model_name, None);
        assert_eq!(record.vendor, None);
        assert_eq!(record.cpu_family, None);
        assert_eq!(record.cpu_mhz, Some(2400));
        assert_eq!(record.virtualization, None);
    }

    #[test]
    fn merge_overrides_non_loongson_name_and_vendor_in_source_order() {
        let merged = merge_cpu_records(
            Some(CpuRecord {
                model_name: Some("Intel Core Ultra".to_string()),
                vendor: Some("GenuineIntel".to_string()),
                ..Default::default()
            }),
            Some(LshwCpuRecord {
                product: Some("Fallback CPU".to_string()),
                vendor: Some("Fallback Vendor".to_string()),
                version: Some("Fallback Version".to_string()),
            }),
            &[DmidecodeCpuRecord {
                manufacturer: Some("DMI Vendor".to_string()),
                version: Some("DMI CPU".to_string()),
                ..Default::default()
            }],
        );

        assert_eq!(merged.name.as_deref(), Some("DMI CPU"));
        assert_eq!(merged.vendor.as_deref(), Some("DMI Vendor"));
    }

    #[test]
    fn merge_corrects_cores_upward_even_when_thread_totals_match() {
        let merged = merge_cpu_records(
            Some(CpuRecord {
                cores_per_socket: Some(16),
                sockets: Some(1),
                threads: Some(96),
                ..Default::default()
            }),
            None,
            &[DmidecodeCpuRecord {
                socket_designation: Some("CPU 0".to_string()),
                core_count: Some(32),
                thread_count: Some(96),
                ..Default::default()
            }],
        );

        assert_eq!(merged.cores, Some(32));
        assert_eq!(merged.threads, Some(96));
    }

    #[test]
    fn merge_uses_lscpu_current_frequency_when_dmi_current_speed_is_missing() {
        let merged = merge_cpu_records(
            Some(CpuRecord {
                cpu_mhz: Some(1800),
                ..Default::default()
            }),
            None,
            &[],
        );

        assert_eq!(merged.current_freq_mhz, Some(1800));
    }

    #[test]
    fn merge_ignores_socket_only_dmi_records_for_socket_and_count_totals() {
        let merged = merge_cpu_records(
            None,
            None,
            &[
                DmidecodeCpuRecord {
                    socket_designation: Some("CPU 0".to_string()),
                    version: Some("Kunpeng 920".to_string()),
                    core_count: Some(48),
                    thread_count: Some(48),
                    ..Default::default()
                },
                DmidecodeCpuRecord {
                    socket_designation: Some("CPU 1".to_string()),
                    ..Default::default()
                },
            ],
        );

        assert_eq!(merged.name.as_deref(), Some("Kunpeng 920"));
        assert_eq!(merged.sockets, Some(1));
        assert_eq!(merged.cores, Some(48));
        assert_eq!(merged.threads, Some(48));
    }

    #[test]
    fn merge_ignores_zero_count_dmi_records_for_socket_and_count_totals() {
        let merged = merge_cpu_records(
            None,
            None,
            &[
                DmidecodeCpuRecord {
                    socket_designation: Some("CPU 0".to_string()),
                    version: Some("Kunpeng 920".to_string()),
                    core_count: Some(48),
                    thread_count: Some(48),
                    ..Default::default()
                },
                DmidecodeCpuRecord {
                    socket_designation: Some("CPU 1".to_string()),
                    core_count: Some(0),
                    thread_count: Some(0),
                    ..Default::default()
                },
            ],
        );

        assert_eq!(merged.name.as_deref(), Some("Kunpeng 920"));
        assert_eq!(merged.sockets, Some(1));
        assert_eq!(merged.cores, Some(48));
        assert_eq!(merged.threads, Some(48));
    }

    #[test]
    fn merge_ignores_overflowing_dmi_totals() {
        let merged = merge_cpu_records(
            None,
            None,
            &[
                DmidecodeCpuRecord {
                    socket_designation: Some("CPU 0".to_string()),
                    core_count: Some(u32::MAX),
                    thread_count: Some(u32::MAX),
                    ..Default::default()
                },
                DmidecodeCpuRecord {
                    socket_designation: Some("CPU 1".to_string()),
                    core_count: Some(1),
                    thread_count: Some(1),
                    ..Default::default()
                },
            ],
        );

        assert_eq!(merged.sockets, Some(2));
        assert_eq!(merged.cores, None);
        assert_eq!(merged.threads, None);
    }

    #[test]
    fn merge_uses_first_dmi_record_with_each_usable_identity_or_speed_value() {
        let merged = merge_cpu_records(
            None,
            None,
            &[
                DmidecodeCpuRecord {
                    socket_designation: Some("CPU 0".to_string()),
                    ..Default::default()
                },
                DmidecodeCpuRecord {
                    socket_designation: Some("CPU 1".to_string()),
                    manufacturer: Some("HiSilicon".to_string()),
                    version: Some("Kunpeng 920".to_string()),
                    max_speed_mhz: Some(2600),
                    current_speed_mhz: Some(2400),
                    ..Default::default()
                },
            ],
        );

        assert_eq!(merged.name.as_deref(), Some("Kunpeng 920"));
        assert_eq!(merged.vendor.as_deref(), Some("HiSilicon"));
        assert_eq!(merged.max_freq_mhz, Some(2600));
        assert_eq!(merged.current_freq_mhz, Some(2400));
    }
}
