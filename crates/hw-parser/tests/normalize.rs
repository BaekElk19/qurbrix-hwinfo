use hw_parser::normalize::{
    infer_cpu_vendor_from_name, normalize_arch, normalize_cpu_vendor_id, normalize_gpu_vendor,
    normalize_gpu_vendor_id,
};
use hw_testdata::fixture;

struct Case {
    path: String,
    line: usize,
    input: String,
    expected: Option<String>,
}

fn cases(path: &str) -> Vec<Case> {
    fixture(path)
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| (!line.trim().is_empty()).then_some((idx + 1, line)))
        .map(|(line_no, line)| {
            if line.matches('\t').count() != 1 {
                panic!("{path}:{line_no}");
            }

            let (input, expected) = line.split_once('\t').expect("checked tab count");
            let expected = (expected != "<none>").then(|| expected.to_string());
            Case {
                path: path.to_string(),
                line: line_no,
                input: input.to_string(),
                expected,
            }
        })
        .collect()
}

#[test]
fn normalizes_arch_aliases() {
    for case in cases("normalize/arch.cases.txt") {
        assert_eq!(
            normalize_arch(&case.input).map(str::to_string),
            case.expected,
            "{}:{} {}",
            case.path,
            case.line,
            case.input
        );
    }
}

#[test]
fn normalizes_cpu_vendor_ids() {
    for case in cases("normalize/cpu-vendor-id.cases.txt") {
        assert_eq!(
            normalize_cpu_vendor_id(&case.input).map(str::to_string),
            case.expected,
            "{}:{} {}",
            case.path,
            case.line,
            case.input
        );
    }
}

#[test]
fn infers_cpu_vendor_from_model_name() {
    for case in cases("normalize/cpu-name-inference.cases.txt") {
        assert_eq!(
            infer_cpu_vendor_from_name(&case.input).map(str::to_string),
            case.expected,
            "{}:{} {}",
            case.path,
            case.line,
            case.input
        );
    }
}

#[test]
fn normalizes_gpu_vendors() {
    for case in cases("normalize/gpu-vendor.cases.txt") {
        assert_eq!(
            normalize_gpu_vendor(&case.input).map(str::to_string),
            case.expected,
            "{}:{} {}",
            case.path,
            case.line,
            case.input
        );
    }
}

#[test]
fn normalizes_gpu_vendor_ids() {
    for case in cases("normalize/gpu-vendor-id.cases.txt") {
        assert_eq!(
            normalize_gpu_vendor_id(&case.input).map(str::to_string),
            case.expected,
            "{}:{} {}",
            case.path,
            case.line,
            case.input
        );
    }
}
