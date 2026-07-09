use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CpuRecord {
    pub architecture: Option<String>,
    pub threads: Option<u32>,
    pub online_threads: Option<u32>,
    pub online_cores: Option<u32>,
    pub threads_per_core: Option<u32>,
    pub model_name: Option<String>,
    pub vendor: Option<String>,
    pub cores_per_socket: Option<u32>,
    pub sockets: Option<u32>,
    pub cpu_mhz: Option<u32>,
    pub cpu_max_mhz: Option<u32>,
    pub cpu_min_mhz: Option<u32>,
    pub cpu_family: Option<String>,
    pub cpu_implementer: Option<String>,
    pub cpu_architecture: Option<String>,
    pub cpu_variant: Option<String>,
    pub cpu_part: Option<String>,
    pub cpu_revision: Option<String>,
    pub cpu_model: Option<String>,
    pub stepping: Option<String>,
    pub bogomips: Option<String>,
    pub flags: Vec<String>,
    pub virtualization: Option<String>,
    pub l1d_cache: Option<String>,
    pub l1i_cache: Option<String>,
    pub l2_cache: Option<String>,
    pub l3_cache: Option<String>,
    pub l4_cache: Option<String>,
    pub clflush_size_bytes: Option<u32>,
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
    pub serial_number: Option<String>,
    pub manufacturer: Option<String>,
    pub version: Option<String>,
    pub family: Option<String>,
    pub max_speed_mhz: Option<u32>,
    pub current_speed_mhz: Option<u32>,
    pub external_clock_mhz: Option<u32>,
    pub core_count: Option<u32>,
    pub core_enabled: Option<u32>,
    pub thread_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MergedCpu {
    pub architecture: Option<String>,
    pub threads: Option<u32>,
    pub online_threads: Option<u32>,
    pub online_cores: Option<u32>,
    pub threads_per_core: Option<u32>,
    pub name: Option<String>,
    pub vendor: Option<String>,
    pub cores: Option<u32>,
    pub enabled_cores: Option<u32>,
    pub sockets: Option<u32>,
    pub socket_designations: Vec<String>,
    pub serial_numbers: Vec<String>,
    pub max_freq_mhz: Option<u32>,
    pub min_freq_mhz: Option<u32>,
    pub current_freq_mhz: Option<u32>,
    pub external_clock_mhz: Option<u32>,
    pub family: Option<String>,
    pub cpu_implementer: Option<String>,
    pub cpu_architecture: Option<String>,
    pub cpu_variant: Option<String>,
    pub cpu_part: Option<String>,
    pub cpu_revision: Option<String>,
    pub model: Option<String>,
    pub stepping: Option<String>,
    pub bogomips: Option<String>,
    pub virtualization: Option<String>,
    pub l1d_cache: Option<String>,
    pub l1i_cache: Option<String>,
    pub l2_cache: Option<String>,
    pub l3_cache: Option<String>,
    pub l4_cache: Option<String>,
    pub clflush_size_bytes: Option<u32>,
    pub flags: Vec<String>,
}

pub fn parse_lscpu(input: &str) -> CpuRecord {
    let mut record = CpuRecord::default();
    let mut l1d_shared_cpu_list = None;
    let mut l1i_shared_cpu_list = None;
    let mut l2_shared_cpu_list = None;
    let mut l3_shared_cpu_list = None;
    let mut l4_shared_cpu_list = None;

    for line in input.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "Architecture" => record.architecture = clean_value(value),
            "CPU(s)" => record.threads = clean_value(value).and_then(|value| value.parse().ok()),
            "On-line CPU(s) list" | "Online CPU(s) list" => {
                record.online_threads =
                    clean_value(value).and_then(|value| parse_shared_cpu_count(&value))
            }
            "Model name" | "model name" => record.model_name = clean_value(value),
            "Vendor ID" | "vendor_id" => record.vendor = clean_value(value),
            "Core(s) per socket" => {
                record.cores_per_socket = clean_value(value).and_then(|value| value.parse().ok())
            }
            "Thread(s) per core" => {
                record.threads_per_core = clean_value(value).and_then(|value| value.parse().ok())
            }
            "Socket(s)" => record.sockets = clean_value(value).and_then(|value| value.parse().ok()),
            "CPU MHz" => record.cpu_mhz = parse_mhz(value),
            "CPU max MHz" => record.cpu_max_mhz = parse_mhz(value),
            "CPU min MHz" => record.cpu_min_mhz = parse_mhz(value),
            "CPU family" | "cpu family" => record.cpu_family = clean_value(value),
            "Model" | "model" => record.cpu_model = clean_value(value),
            "Stepping" | "stepping" => record.stepping = clean_value(value),
            "BogoMIPS" | "bogomips" => record.bogomips = clean_value(value),
            "Flags" | "flags" => {
                record.flags = value.split_whitespace().map(str::to_string).collect()
            }
            "Virtualization" => record.virtualization = clean_value(value),
            "L1d cache" => record.l1d_cache = clean_value(value),
            "L1i cache" => record.l1i_cache = clean_value(value),
            "L2 cache" => record.l2_cache = clean_value(value),
            "L3 cache" => record.l3_cache = clean_value(value),
            "L4 cache" => record.l4_cache = clean_value(value),
            key if key.eq_ignore_ascii_case("L1d shared cpu list") => {
                l1d_shared_cpu_list = clean_value(value)
            }
            key if key.eq_ignore_ascii_case("L1i shared cpu list") => {
                l1i_shared_cpu_list = clean_value(value)
            }
            key if key.eq_ignore_ascii_case("L2 shared cpu list") => {
                l2_shared_cpu_list = clean_value(value)
            }
            key if key.eq_ignore_ascii_case("L3 shared cpu list") => {
                l3_shared_cpu_list = clean_value(value)
            }
            key if key.eq_ignore_ascii_case("L4 shared cpu list") => {
                l4_shared_cpu_list = clean_value(value)
            }
            _ => {}
        }
    }
    let inferred_cores =
        infer_cores_per_socket(record.threads, record.threads_per_core, record.sockets);
    if let Some(inferred_cores) = inferred_cores {
        let topology_matches = record
            .cores_per_socket
            .zip(record.threads_per_core)
            .zip(record.sockets)
            .and_then(|((cores, threads_per_core), sockets)| {
                cores.checked_mul(threads_per_core)?.checked_mul(sockets)
            })
            == record.threads;
        if record.cores_per_socket.is_none() || !topology_matches {
            record.cores_per_socket = Some(inferred_cores);
        }
    }
    record.online_cores = infer_online_cores(record.online_threads, record.threads_per_core);
    let cores = record
        .cores_per_socket
        .zip(record.sockets)
        .and_then(|(cores, sockets)| cores.checked_mul(sockets));
    normalize_lscpu_cache(
        &mut record.l1d_cache,
        l1d_shared_cpu_list.as_deref(),
        record.threads,
        cores,
    );
    normalize_lscpu_cache(
        &mut record.l1i_cache,
        l1i_shared_cpu_list.as_deref(),
        record.threads,
        cores,
    );
    normalize_lscpu_cache(
        &mut record.l2_cache,
        l2_shared_cpu_list.as_deref(),
        record.threads,
        cores,
    );
    normalize_lscpu_cache(
        &mut record.l3_cache,
        l3_shared_cpu_list.as_deref(),
        record.threads,
        Some(1),
    );
    normalize_lscpu_cache(
        &mut record.l4_cache,
        l4_shared_cpu_list.as_deref(),
        record.threads,
        Some(1),
    );
    record
}

pub fn parse_proc_cpuinfo(input: &str) -> CpuRecord {
    let mut record = CpuRecord::default();
    let mut processor_count = 0u32;
    let mut physical_ids = Vec::new();
    let mut siblings_per_socket = None;

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
            "physical id" => {
                if let Some(value) = clean_value(value) {
                    if !physical_ids.contains(&value) {
                        physical_ids.push(value);
                    }
                }
            }
            "cpu cores" => {
                if record.cores_per_socket.is_none() {
                    record.cores_per_socket =
                        clean_value(value).and_then(|value| value.parse().ok());
                }
            }
            "siblings" => {
                if siblings_per_socket.is_none() {
                    siblings_per_socket = clean_value(value).and_then(|value| value.parse().ok());
                }
            }
            "cpu MHz" | "CPU MHz" | "cpu mhz" => {
                if record.cpu_mhz.is_none() {
                    record.cpu_mhz = parse_mhz(value);
                }
            }
            "BogoMIPS" | "bogomips" => assign_if_empty(&mut record.bogomips, clean_value(value)),
            "cache size" => assign_if_empty(&mut record.l2_cache, clean_value(value)),
            "clflush size" => {
                record.clflush_size_bytes = record
                    .clflush_size_bytes
                    .or_else(|| clean_value(value).and_then(|value| value.parse().ok()));
            }
            "flags" | "Features" | "features" => {
                if record.flags.is_empty() {
                    record.flags = value.split_whitespace().map(str::to_string).collect();
                }
            }
            "CPU implementer" => assign_if_empty(&mut record.cpu_implementer, clean_value(value)),
            "CPU architecture" => {
                assign_if_empty(&mut record.cpu_architecture, clean_value(value));
                assign_if_empty(&mut record.architecture, proc_architecture(value));
            }
            "CPU variant" => assign_if_empty(&mut record.cpu_variant, clean_value(value)),
            "CPU part" => assign_if_empty(&mut record.cpu_part, clean_value(value)),
            "CPU revision" => assign_if_empty(&mut record.cpu_revision, clean_value(value)),
            "Architecture" => {
                assign_if_empty(&mut record.architecture, proc_architecture(value));
            }
            _ => {}
        }
    }

    if processor_count > 0 {
        record.threads = Some(processor_count);
    }
    record.sockets = (!physical_ids.is_empty())
        .then_some(physical_ids.len() as u32)
        .or_else(|| infer_sockets_from_siblings(record.threads, siblings_per_socket))
        .or_else(|| record.cores_per_socket.map(|_| 1));
    if record.threads.is_none() {
        record.threads = siblings_per_socket
            .zip(record.sockets)
            .and_then(|(siblings, sockets)| siblings.checked_mul(sockets))
            .or(siblings_per_socket);
    }
    record.threads_per_core = infer_threads_per_core(
        record.threads,
        record.cores_per_socket,
        record.sockets,
        siblings_per_socket,
    );

    record
}

pub fn parse_proc_hardware(input: &str) -> CpuRecord {
    let mut record = CpuRecord::default();
    let input_lc = input.to_ascii_lowercase();

    if input.contains("HUAWEI Kirin 9006C") {
        record.model_name = Some("HUAWEI Kirin 9006C".to_string());
    } else if input.contains("HUAWEI Kirin 990") || input_lc.contains("kirin990") {
        record.model_name = Some("HUAWEI Kirin 990".to_string());
    } else if input.contains("PANGU M900") || input_lc.contains("pangu m900") {
        // Deepin/Phytium PANGU M900 board: /proc/hardware carries the SoC id.
        record.model_name = Some("Phytium PANGU M900".to_string());
    } else if input.contains("HW990") || input_lc.contains("hw990") {
        // Kylin/Huawei HW990 variant (`/proc/hardware`), matching
        // `Judgment_HW990()` in rubbish-clear-dbus/src/detailinfo/cpuinfo.py.
        record.model_name = Some("Huawei HW990".to_string());
    } else if input_lc.contains("phytium ft-1500a")
        || input_lc.contains("ft-1500a")
        || input_lc.contains("ft1500a")
    {
        record.model_name = Some("Phytium FT-1500A".to_string());
    } else if input_lc.contains("phytium ft-2000") || input_lc.contains("ft-2000") {
        record.model_name = Some("Phytium FT-2000".to_string());
    } else if input_lc.contains("phytium d2000") || input_lc.contains("d2000") {
        record.model_name = Some("Phytium D2000".to_string());
    }

    record
}

/// Deepin's `Common::isHwPlatform()` check.
pub fn is_hw_platform_marker(vendor: Option<&str>, product: Option<&str>) -> bool {
    let is_hw = |value: Option<&str>| -> bool {
        value
            .map(|value| {
                let lc = value.to_ascii_lowercase();
                lc.contains("huawei")
                    || lc.contains("hisilicon")
                    || lc.contains("kunpeng")
                    || lc.contains("pangu")
                    || lc.contains("pguw")
                    || lc.contains("klvv")
                    || lc.contains("klvu")
                    || lc.contains("pguv")
                    || lc.contains("hw990")
                    || lc.contains("kirin")
            })
            .unwrap_or(false)
    };
    is_hw(vendor) || is_hw(product)
}

/// Parse the Kylin `/sys/phytium1500a_info` file. Only present on Phytium 1500a
/// boards; on Kylin the file gates the FT-1500a fallback path.
pub fn parse_phytium1500a_info(input: &str) -> CpuRecord {
    let mut record = CpuRecord::default();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return record;
    }
    record.model_name = Some("Phytium FT-1500A".to_string());
    record.vendor = Some("Phytium".to_string());
    record.architecture = Some("aarch64".to_string());
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let lc = line.to_ascii_lowercase();
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            let key_lc = key.trim().to_ascii_lowercase();
            if key_lc.contains("model") || key_lc.contains("name") {
                if let Some(clean) = clean_value(value) {
                    record.model_name = Some(clean);
                }
            } else if key_lc.contains("freq") || key_lc.contains("speed") {
                if let Some(mhz) = parse_mhz(value) {
                    record.cpu_max_mhz = record.cpu_max_mhz.or(Some(mhz));
                }
            }
        } else if lc.contains("mhz") || lc.contains("ghz") {
            if let Some(mhz) = parse_mhz(line) {
                record.cpu_max_mhz = record.cpu_max_mhz.or(Some(mhz));
            }
        }
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
            "Serial Number" => record.serial_number = clean_value(value),
            "Manufacturer" => record.manufacturer = clean_value(value),
            "Version" => record.version = clean_value(value),
            "Family" => record.family = clean_value(value),
            "Max Speed" => record.max_speed_mhz = parse_mhz(value),
            "Current Speed" => record.current_speed_mhz = parse_mhz(value),
            "External Clock" => record.external_clock_mhz = parse_mhz(value),
            "Core Count" => record.core_count = value.parse().ok(),
            "Core Enabled" => record.core_enabled = value.parse().ok(),
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
    let dmi_external_clock_mhz = useful_dmi
        .iter()
        .find_map(|record| record.external_clock_mhz);
    let dmi_family = dmi.iter().find_map(|record| record.family.clone());
    let dmi_enabled_cores =
        sum_dmi_count(useful_dmi.iter().filter_map(|record| record.core_enabled))
            .filter(|value| *value > 0 && *value <= 512);
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
    name = name.map(clean_cpu_name);

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
        online_threads: lscpu.as_ref().and_then(|record| record.online_threads),
        online_cores: lscpu.as_ref().and_then(|record| record.online_cores),
        threads_per_core: lscpu.as_ref().and_then(|record| record.threads_per_core),
        name,
        vendor,
        cores,
        enabled_cores: dmi_enabled_cores,
        sockets,
        socket_designations: unique_dmi_strings(
            useful_dmi
                .iter()
                .filter_map(|record| record.socket_designation.as_deref()),
        ),
        serial_numbers: unique_dmi_strings(
            useful_dmi
                .iter()
                .filter_map(|record| record.serial_number.as_deref()),
        ),
        max_freq_mhz: lscpu
            .as_ref()
            .and_then(|record| record.cpu_max_mhz)
            .or(dmi_max_speed_mhz),
        min_freq_mhz: lscpu.as_ref().and_then(|record| record.cpu_min_mhz),
        current_freq_mhz: dmi_current_speed_mhz
            .or_else(|| lscpu.as_ref().and_then(|record| record.cpu_mhz)),
        external_clock_mhz: dmi_external_clock_mhz,
        family: lscpu
            .as_ref()
            .and_then(|record| record.cpu_family.clone())
            .or(dmi_family),
        cpu_implementer: lscpu
            .as_ref()
            .and_then(|record| record.cpu_implementer.clone()),
        cpu_architecture: lscpu
            .as_ref()
            .and_then(|record| record.cpu_architecture.clone()),
        cpu_variant: lscpu.as_ref().and_then(|record| record.cpu_variant.clone()),
        cpu_part: lscpu.as_ref().and_then(|record| record.cpu_part.clone()),
        cpu_revision: lscpu
            .as_ref()
            .and_then(|record| record.cpu_revision.clone()),
        model: lscpu.as_ref().and_then(|record| record.cpu_model.clone()),
        stepping: lscpu.as_ref().and_then(|record| record.stepping.clone()),
        bogomips: lscpu.as_ref().and_then(|record| record.bogomips.clone()),
        virtualization: lscpu
            .as_ref()
            .and_then(|record| record.virtualization.clone()),
        l1d_cache: lscpu.as_ref().and_then(|record| record.l1d_cache.clone()),
        l1i_cache: lscpu.as_ref().and_then(|record| record.l1i_cache.clone()),
        l2_cache: lscpu.as_ref().and_then(|record| record.l2_cache.clone()),
        l3_cache: lscpu.as_ref().and_then(|record| record.l3_cache.clone()),
        l4_cache: lscpu.as_ref().and_then(|record| record.l4_cache.clone()),
        clflush_size_bytes: lscpu.as_ref().and_then(|record| record.clflush_size_bytes),
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
            || self.serial_number.is_some()
            || self.max_speed_mhz.is_some_and(|value| value > 0)
            || self.current_speed_mhz.is_some_and(|value| value > 0)
            || self.external_clock_mhz.is_some_and(|value| value > 0)
            || self.core_count.is_some_and(|value| value > 0)
            || self.core_enabled.is_some_and(|value| value > 0)
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

fn unique_dmi_strings<'a>(values: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values.map(str::trim).filter(|value| !value.is_empty()) {
        if !unique.iter().any(|seen| seen == value) {
            unique.push(value.to_string());
        }
    }
    unique
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

fn clean_cpu_name(value: String) -> String {
    let value = value.trim();
    let stripped = if value.to_ascii_lowercase().contains("loongson") {
        crate::normalize::cpu_vendor::clean_loongson_name(value)
    } else {
        value.to_string()
    };
    let stripped_ref = stripped.as_str();
    for marker in [" x ", " X "] {
        if let Some((prefix, suffix)) = stripped_ref.rsplit_once(marker) {
            if !prefix.trim().is_empty()
                && !suffix.is_empty()
                && suffix.chars().all(|ch| ch.is_ascii_digit())
            {
                return prefix.trim_end().to_string();
            }
        }
    }
    if stripped_ref
        .chars()
        .next_back()
        .is_some_and(|ch| ch.is_ascii_digit())
    {
        if let Some(idx) = stripped_ref.rfind('/') {
            let (prefix, suffix) = stripped_ref.split_at(idx);
            let suffix_body = suffix.trim_start_matches('/');
            if !suffix_body.is_empty() && suffix_body.chars().all(|ch| ch.is_ascii_digit()) {
                return prefix.trim_end().to_string();
            }
        }
    }
    stripped
}

fn parse_mhz(value: &str) -> Option<u32> {
    value
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<f32>().ok())
        .map(|value| value.round() as u32)
}

fn infer_cores_per_socket(
    threads: Option<u32>,
    threads_per_core: Option<u32>,
    sockets: Option<u32>,
) -> Option<u32> {
    let threads = threads?;
    let divisor = threads_per_core?.checked_mul(sockets?)?;
    (divisor > 0 && threads % divisor == 0)
        .then(|| threads / divisor)
        .filter(|value| *value > 0)
}

fn infer_online_cores(online_threads: Option<u32>, threads_per_core: Option<u32>) -> Option<u32> {
    let online_threads = online_threads?;
    let threads_per_core = threads_per_core?.max(1);
    online_threads
        .checked_add(threads_per_core.checked_sub(1)?)?
        .checked_div(threads_per_core)
        .filter(|value| *value > 0)
}

fn infer_sockets_from_siblings(
    threads: Option<u32>,
    siblings_per_socket: Option<u32>,
) -> Option<u32> {
    let threads = threads?;
    let siblings = siblings_per_socket?;
    (siblings > 0 && threads % siblings == 0)
        .then(|| threads / siblings)
        .filter(|value| *value > 0)
}

fn infer_threads_per_core(
    threads: Option<u32>,
    cores_per_socket: Option<u32>,
    sockets: Option<u32>,
    siblings_per_socket: Option<u32>,
) -> Option<u32> {
    if let Some(value) = siblings_per_socket
        .zip(cores_per_socket)
        .and_then(|(siblings, cores)| {
            (cores > 0 && siblings % cores == 0).then(|| siblings / cores)
        })
        .filter(|value| *value > 0)
    {
        return Some(value);
    }

    let divisor = sockets?.checked_mul(cores_per_socket?)?;
    let threads = threads?;
    (divisor > 0 && threads % divisor == 0)
        .then(|| threads / divisor)
        .filter(|value| *value > 0)
}

pub fn cpu_extensions_from_flags(flags: &[String]) -> Vec<String> {
    let flags: Vec<_> = flags.iter().map(|flag| flag.to_ascii_lowercase()).collect();
    let has = |needle: &str| flags.iter().any(|flag| flag == needle);
    let has_prefix = |prefix: &str| flags.iter().any(|flag| flag.starts_with(prefix));
    let has_avx512 = flags.iter().any(|flag| flag.starts_with("avx512"));
    let mut extensions = Vec::new();

    for (present, name) in [
        (has("mmx"), "MMX"),
        (has("sse"), "SSE"),
        (has("sse2"), "SSE2"),
        (has("sse3"), "SSE3"),
        (has("3dnow"), "3D Now"),
        (has_prefix("sse4"), "SSE4"),
        (has("ssse3"), "SSSE3"),
        (has("sse4_1"), "SSE4_1"),
        (has("sse4_2"), "SSE4_2"),
        (has("amd64"), "AMD64"),
        (has("em64t") || has("lm"), "EM64T"),
        (has("avx"), "AVX"),
        (has("avx2"), "AVX2"),
        (has_avx512, "AVX512"),
        (has("fma"), "FMA"),
        (has("aes"), "AES"),
        (has("sha_ni") || has("sha1_ni") || has("sha2_ni"), "SHA"),
        (has("vmx"), "VMX"),
        (has("svm"), "SVM"),
        // ARMv8 crypto extensions surface on aarch64 as "aes"/"sha1"/"sha2"
        (has("sha1"), "SHA1"),
        (has("sha2"), "SHA2"),
        (has("crc32"), "CRC32"),
        // Deepin Overview groups NEON under "Extensions" too
        (has("neon") || has("asimd"), "NEON"),
        (has("crypto"), "Crypto"),
        (has("rdrand"), "RDRAND"),
        (has("rdseed"), "RDSEED"),
    ] {
        if present && !extensions.iter().any(|value| value == name) {
            extensions.push(name.to_string());
        }
    }

    extensions
}

/// Deepin `DeviceCpu::setInfoFromLscpu()` frequency-range renderer. Returns
/// `(display_string, is_range)` matching the "%1-%2 GHz" / "%1 GHz" / "%1 MHz"
/// formatting used by the deepin-devicemanager UI. Prefers min/max range when
/// both are available and non-equal; otherwise renders the higher of
/// `max_freq_mhz` / `current_freq_mhz`.
pub fn format_cpu_frequency_display(
    max_mhz: Option<u32>,
    min_mhz: Option<u32>,
    current_mhz: Option<u32>,
) -> (Option<String>, bool) {
    if let (Some(min), Some(max)) = (min_mhz, max_mhz) {
        let min_ghz = f64::from(min) / 1000.0;
        let max_ghz = f64::from(max) / 1000.0;
        if (min_ghz - max_ghz).abs() < 0.001 {
            return (Some(format_freq_ghz_or_mhz(max_ghz)), false);
        }
        return (
            Some(format!(
                "{}-{} GHz",
                trim_trailing_zero(&format!("{:.3}", min_ghz)),
                trim_trailing_zero(&format!("{:.3}", max_ghz))
            )),
            true,
        );
    }
    match max_mhz.or(current_mhz) {
        Some(mhz) => (Some(format_freq_ghz_or_mhz(f64::from(mhz) / 1000.0)), false),
        None => (None, false),
    }
}

fn format_freq_ghz_or_mhz(ghz: f64) -> String {
    if ghz > 1.0 {
        format!("{} GHz", trim_trailing_zero(&format!("{:.3}", ghz)))
    } else {
        let mhz = ghz * 1000.0;
        format!("{} MHz", trim_trailing_zero(&format!("{:.1}", mhz)))
    }
}

fn trim_trailing_zero(value: &str) -> String {
    if !value.contains('.') {
        return value.to_string();
    }
    let trimmed = value.trim_end_matches('0');
    trimmed.trim_end_matches('.').to_string()
}

/// Deepin `DeviceCpu::getOverviewInfo()` renders `"{name} ({cores} Core(s) / {threads} Processor)"`
/// with English word counts when a mapping is available (matching the m_trNumber table),
/// otherwise falls back to digits.
pub fn format_cpu_overview(name: &str, cores: Option<u32>, threads: Option<u32>) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let cores = cores?;
    let threads = threads?;
    if cores == 0 && threads == 0 {
        return None;
    }
    Some(format!(
        "{} ({} Core(s) / {} Processor)",
        name,
        english_or_digits(cores),
        english_or_digits(threads),
    ))
}

/// Deepin `DeviceCpu.cpp:11-79` `m_trNumber` mapping.
/// rather than "48". Only maps counts Deepin explicitly enumerates; unknown
/// counts fall through to the decimal string.
pub fn cpu_count_english_word(count: u32) -> Option<&'static str> {
    Some(match count {
        1 => "One",
        2 => "Two",
        3 => "Three",
        4 => "Four",
        5 => "Five",
        6 => "Six",
        7 => "Seven",
        8 => "Eight",
        9 => "Nine",
        10 => "Ten",
        11 => "Eleven",
        12 => "Twelve",
        14 => "Fourteen",
        16 => "Sixteen",
        18 => "Eighteen",
        20 => "Twenty",
        22 => "Twenty-two",
        24 => "Twenty-four",
        26 => "Twenty-six",
        28 => "Twenty-eight",
        30 => "Thirty",
        32 => "Thirty-two",
        36 => "Thirty-six",
        40 => "Forty",
        44 => "Forty-four",
        48 => "Forty-eight",
        56 => "Fifty-six",
        60 => "Sixty",
        64 => "Sixty-four",
        72 => "Seventy-two",
        80 => "Eighty",
        88 => "Eighty-eight",
        96 => "Ninety-six",
        112 => "One hundred and Twelve",
        120 => "One hundred and Twenty",
        128 => "One hundred and Twenty-eight",
        144 => "One hundred and Forty-four",
        160 => "One hundred and Sixty",
        176 => "One hundred and Seventy-six",
        192 => "One hundred and Ninety-two",
        224 => "Two hundred and Twenty-four",
        256 => "Two hundred and Fifty-six",
        _ => return None,
    })
}

fn english_or_digits(count: u32) -> String {
    cpu_count_english_word(count)
        .map(str::to_string)
        .unwrap_or_else(|| count.to_string())
}

fn normalize_lscpu_cache(
    cache: &mut Option<String>,
    shared_cpu_list: Option<&str>,
    threads: Option<u32>,
    fallback_groups: Option<u32>,
) {
    let Some(cache_value) = cache.as_deref() else {
        return;
    };
    if cache_value.contains("instance") {
        return;
    }
    let Some(shared_count) = shared_cpu_list.and_then(parse_shared_cpu_count) else {
        return;
    };
    let groups = threads
        .and_then(|threads| threads.checked_div(shared_count))
        .filter(|groups| *groups > 0)
        .or(fallback_groups)
        .unwrap_or(1);
    let Some(total) = format_total_cache(cache_value, groups) else {
        return;
    };
    *cache = Some(total);
}

fn parse_shared_cpu_count(shared_cpu_list: &str) -> Option<u32> {
    let mut count = 0u32;
    for part in shared_cpu_list.split(',').map(str::trim) {
        if part.is_empty() {
            continue;
        }
        let add = if let Some((start, end)) = part.split_once('-') {
            let start = start.trim().parse::<u32>().ok()?;
            let end = end.trim().parse::<u32>().ok()?;
            end.checked_sub(start)?.checked_add(1)?
        } else {
            part.parse::<u32>().ok()?;
            1
        };
        count = count.checked_add(add)?;
    }
    (count > 0).then_some(count)
}

fn format_total_cache(cache: &str, groups: u32) -> Option<String> {
    let total_kib = parse_cache_kib(cache)? * f64::from(groups);
    let (value, unit) = if total_kib >= 1024.0 * 1024.0 {
        (total_kib / (1024.0 * 1024.0), "GiB")
    } else if total_kib >= 1024.0 {
        (total_kib / 1024.0, "MiB")
    } else {
        (total_kib, "KiB")
    };

    if (value.fract()).abs() < 1e-6 {
        Some(format!("{value:.0} {unit}"))
    } else {
        Some(format!("{value:.1} {unit}"))
    }
}

fn parse_cache_kib(cache: &str) -> Option<f64> {
    let cache = cache.trim();
    let number_end = cache
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .unwrap_or(cache.len());
    let value = cache.get(..number_end)?.parse::<f64>().ok()?;
    let unit = cache.get(number_end..)?.trim().to_ascii_lowercase();

    if unit.starts_with('k') {
        Some(value)
    } else if unit.starts_with('m') {
        Some(value * 1024.0)
    } else if unit.starts_with('g') {
        Some(value * 1024.0 * 1024.0)
    } else if unit.starts_with('t') {
        Some(value * 1024.0 * 1024.0 * 1024.0)
    } else if unit.is_empty() || unit.starts_with('b') {
        Some(value / 1024.0)
    } else {
        Some(value)
    }
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
    let placeholder = [
        "N/A",
        "Null",
        "none",
        "Not Provided",
        "Not Specified",
        "Default string",
        "Unspecified",
        "Unknown",
    ]
    .iter()
    .any(|placeholder| value.eq_ignore_ascii_case(placeholder));
    (!value.is_empty() && !placeholder).then(|| value.to_string())
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
    fn parse_cpu_sources_clean_deepin_placeholder_values() {
        let record = parse_lscpu(
            "Architecture: aarch64\n\
             Model name: N/A\n\
             Vendor ID: Null\n\
             CPU family: none\n\
             CPU MHz: 2400.4 MHz\n\
             Virtualization: Default string\n",
        );

        assert_eq!(record.architecture.as_deref(), Some("aarch64"));
        assert_eq!(record.model_name, None);
        assert_eq!(record.vendor, None);
        assert_eq!(record.cpu_family, None);
        assert_eq!(record.cpu_mhz, Some(2400));
        assert_eq!(record.virtualization, None);

        let proc = parse_proc_cpuinfo(
            "processor\t: 0\n\
             model name\t: Not Provided\n\
             Hardware\t: Unspecified\n\
             CPU implementer\t: N/A\n\
             CPU architecture\t: Default string\n\
             cache size\t: Null\n",
        );

        assert_eq!(proc.model_name, None);
        assert_eq!(proc.cpu_implementer, None);
        assert_eq!(proc.cpu_architecture, None);
        assert_eq!(proc.architecture, None);
        assert_eq!(proc.l2_cache, None);

        let dmi = parse_dmidecode_processor(
            "Handle 0x0041, DMI type 4, 48 bytes\n\
             Processor Information\n\
             \tSocket Designation: Default string\n\
             \tSerial Number: none\n\
             \tManufacturer: Not Provided\n\
             \tVersion: N/A\n\
             \tFamily: Null\n",
        );

        assert_eq!(dmi.len(), 1);
        assert_eq!(dmi[0].socket_designation, None);
        assert_eq!(dmi[0].serial_number, None);
        assert_eq!(dmi[0].manufacturer, None);
        assert_eq!(dmi[0].version, None);
        assert_eq!(dmi[0].family, None);
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

    #[test]
    fn parse_proc_hardware_detects_pangu_m900_and_hw990() {
        assert_eq!(
            parse_proc_hardware("Hardware\t: Phytium PANGU M900")
                .model_name
                .as_deref(),
            Some("Phytium PANGU M900"),
        );
        assert_eq!(
            parse_proc_hardware("Hardware\t: HUAWEI HW990")
                .model_name
                .as_deref(),
            Some("Huawei HW990"),
        );
        assert_eq!(
            parse_proc_hardware("Hardware\t: Phytium FT-1500A/16")
                .model_name
                .as_deref(),
            Some("Phytium FT-1500A"),
        );
        assert_eq!(
            parse_proc_hardware("Hardware\t: Phytium D2000/8")
                .model_name
                .as_deref(),
            Some("Phytium D2000"),
        );
    }

    #[test]
    fn parse_phytium1500a_info_reports_ft_1500a_identity() {
        let record = parse_phytium1500a_info("Phytium FT-1500A/16\nfreq: 1500 MHz\n");
        assert_eq!(record.model_name.as_deref(), Some("Phytium FT-1500A"));
        assert_eq!(record.vendor.as_deref(), Some("Phytium"));
        assert_eq!(record.architecture.as_deref(), Some("aarch64"));
        assert_eq!(record.cpu_max_mhz, Some(1500));
    }

    #[test]
    fn parse_phytium1500a_info_empty_returns_default() {
        let record = parse_phytium1500a_info("");
        assert!(record.is_empty());
    }

    #[test]
    fn is_hw_platform_marker_detects_huawei_and_pangu() {
        assert!(is_hw_platform_marker(Some("Huawei"), None));
        assert!(is_hw_platform_marker(None, Some("PANGU M900")));
        assert!(is_hw_platform_marker(
            Some("HiSilicon"),
            Some("Kunpeng-920")
        ));
        assert!(is_hw_platform_marker(None, Some("KLVV Board")));
        assert!(!is_hw_platform_marker(Some("Intel"), Some("NUC")));
        assert!(!is_hw_platform_marker(None, None));
    }

    #[test]
    fn cpu_extensions_from_flags_includes_avx_aes_sha_and_virt() {
        let flags: Vec<String> = [
            "mmx", "sse", "sse2", "ssse3", "sse4_1", "sse4_2", "avx", "avx2", "avx512f", "aes",
            "sha_ni", "vmx", "svm", "lm",
        ]
        .iter()
        .map(|value| value.to_string())
        .collect();
        let extensions = cpu_extensions_from_flags(&flags);
        for name in [
            "MMX", "SSE", "SSE2", "SSSE3", "SSE4_1", "SSE4_2", "SSE4", "AVX", "AVX2", "AVX512",
            "AES", "SHA", "VMX", "SVM", "EM64T",
        ] {
            assert!(
                extensions.iter().any(|value| value == name),
                "missing {}",
                name
            );
        }
    }

    #[test]
    fn format_cpu_frequency_display_renders_range_and_single_values() {
        // Range case (min != max, difference > 0.001 GHz)
        let (display, is_range) = format_cpu_frequency_display(Some(3200), Some(1200), None);
        assert!(is_range);
        assert_eq!(display.as_deref(), Some("1.2-3.2 GHz"));

        // Collapsed range (min == max)
        let (display, is_range) = format_cpu_frequency_display(Some(2400), Some(2400), None);
        assert!(!is_range);
        assert_eq!(display.as_deref(), Some("2.4 GHz"));

        // Only max
        let (display, is_range) = format_cpu_frequency_display(Some(1800), None, None);
        assert!(!is_range);
        assert_eq!(display.as_deref(), Some("1.8 GHz"));

        // Current-only fallback
        let (display, is_range) = format_cpu_frequency_display(None, None, Some(800));
        assert!(!is_range);
        assert_eq!(display.as_deref(), Some("800 MHz"));

        // Nothing at all
        let (display, is_range) = format_cpu_frequency_display(None, None, None);
        assert_eq!(display, None);
        assert!(!is_range);
    }

    #[test]
    fn format_cpu_overview_uses_english_word_counts_when_available() {
        assert_eq!(
            format_cpu_overview("Kunpeng 920", Some(48), Some(48)).as_deref(),
            Some("Kunpeng 920 (Forty-eight Core(s) / Forty-eight Processor)"),
        );
        // Falls back to digits for counts not in the mapping table
        assert_eq!(
            format_cpu_overview("Intel Xeon Gold", Some(13), Some(26)).as_deref(),
            Some("Intel Xeon Gold (13 Core(s) / Twenty-six Processor)"),
        );
    }

    #[test]
    fn format_cpu_overview_returns_none_when_data_missing() {
        assert_eq!(format_cpu_overview("", Some(8), Some(16)), None);
        assert_eq!(format_cpu_overview("CPU", None, Some(8)), None);
        assert_eq!(format_cpu_overview("CPU", Some(0), Some(0)), None);
    }

    #[test]
    fn merge_cleans_loongson_name_trailing_at_frequency() {
        let merged = merge_cpu_records(
            Some(CpuRecord {
                model_name: Some("Loongson-3A5000 @ 2500.00MHz".to_string()),
                ..Default::default()
            }),
            None,
            &[],
        );
        assert_eq!(merged.name.as_deref(), Some("Loongson-3A5000"));
    }
}
