use std::{fs, path::Path};

#[test]
fn legacy_placeholder_crates_are_not_part_of_the_workspace() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for crate_name in ["hw-api", "hw-store", "hw-merge"] {
        assert!(
            !root.join("crates").join(crate_name).exists(),
            "{crate_name} should be created only when it has real code"
        );
    }

    let lock = fs::read_to_string(root.join("Cargo.lock")).expect("Cargo.lock should be readable");
    for crate_name in ["hw-api", "hw-store", "hw-merge"] {
        assert!(
            !lock.contains(&format!("name = \"{crate_name}\"")),
            "{crate_name} should not remain in Cargo.lock"
        );
    }
}
