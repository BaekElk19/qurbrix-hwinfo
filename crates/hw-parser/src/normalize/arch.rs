pub fn normalize_arch(uname_m: &str) -> Option<&'static str> {
    match uname_m.trim().to_ascii_lowercase().as_str() {
        "x86_64" | "amd64" => Some("x86_64"),
        "i386" | "i686" => Some("i386"),
        "aarch64" | "arm64" => Some("aarch64"),
        "loongarch64" | "loongarch" => Some("loongarch64"),
        "sw_64" => Some("sw_64"),
        "mips64" | "mips64el" => Some("mips64"),
        "riscv64" => Some("riscv64"),
        _ => None,
    }
}
