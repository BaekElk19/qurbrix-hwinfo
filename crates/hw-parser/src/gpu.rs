use crate::{parse_lspci_nn_k, PciRecord};

pub fn parse_gpu_lspci(input: &str) -> Vec<PciRecord> {
    parse_lspci_nn_k(input)
        .into_iter()
        .filter(|record| {
            let class = record
                .class_name
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase();
            class.contains("vga") || class.contains("3d controller") || class.contains("display")
        })
        .collect()
}
