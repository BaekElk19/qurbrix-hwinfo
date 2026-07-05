use std::path::{Path, PathBuf};

pub fn fixture_path(relative: impl AsRef<Path>) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(relative)
}

pub fn fixture(relative: impl AsRef<Path>) -> String {
    std::fs::read_to_string(fixture_path(relative)).expect("fixture exists")
}

pub fn fixture_bytes(relative: impl AsRef<Path>) -> Vec<u8> {
    std::fs::read(fixture_path(relative)).expect("fixture exists")
}
