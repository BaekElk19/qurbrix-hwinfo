use hw_model::{
    CoreIdentityGroup, EnsureSnapshotOptions, IdentityCoverage, PartialPolicy, SnapshotId,
    BINDID_V2_ALGORITHM, FINGERPRINT_VERSION, SNAPSHOT_SCHEMA_VERSION,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, str::FromStr};

#[derive(Deserialize)]
struct GoldenFile {
    schema_version: String,
    fingerprint_version: u32,
    vectors: Vec<GoldenVector>,
}

#[derive(Deserialize)]
struct GoldenVector {
    identity_records: Vec<String>,
    configuration_records: Vec<String>,
    machine_bind_id: String,
    configuration_fingerprint: String,
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[test]
fn snapshot_id_contract_is_lowercase_uuid_v7() {
    let id = SnapshotId::new_v7();
    let encoded = id.to_string();
    assert_eq!(encoded, encoded.to_ascii_lowercase());
    assert_eq!(encoded.len(), 36);
    assert_eq!(SnapshotId::from_str(&encoded).unwrap(), id);
    assert_eq!(id.as_uuid().get_version_num(), 7);
}

#[test]
fn defaults_match_the_accepted_contract() {
    let options = EnsureSnapshotOptions::default();
    assert_eq!(options.partial_policy, PartialPolicy::PublishIfCoreComplete);
    assert_eq!(options.max_snapshot_age.unwrap().as_secs(), 86_400);
    assert_eq!(SNAPSHOT_SCHEMA_VERSION, "qurbrix.hw.snapshot.v1");
    assert_eq!(BINDID_V2_ALGORITHM, "qurbrix-hw-bindid-sha256-v2");
}

#[test]
fn core_completeness_accepts_trusted_absence_but_not_failure() {
    let complete = IdentityCoverage {
        covered: vec![
            CoreIdentityGroup::Platform,
            CoreIdentityGroup::Cpu,
            CoreIdentityGroup::Memory,
            CoreIdentityGroup::Storage,
        ],
        missing: Vec::new(),
        trusted_absent: vec![CoreIdentityGroup::PhysicalNetwork],
    };
    assert!(complete.core_complete());

    let failed = IdentityCoverage {
        missing: vec![CoreIdentityGroup::PhysicalNetwork],
        trusted_absent: Vec::new(),
        ..complete
    };
    assert!(!failed.core_complete());
}

#[test]
fn golden_vectors_recompute_byte_for_byte() {
    let golden: GoldenFile = serde_json::from_str(include_str!(
        "../docs/hardware-snapshot-golden-vectors.json"
    ))
    .unwrap();
    assert_eq!(golden.schema_version, "qurbrix.hw.snapshot.contract.v1");
    assert_eq!(golden.fingerprint_version, FINGERPRINT_VERSION);

    for vector in golden.vectors {
        let mut identities = vector.identity_records;
        identities.sort();
        identities.dedup();
        let machine_payload = serde_json::to_vec(&identities).unwrap();
        assert_eq!(sha256_hex(&machine_payload), vector.machine_bind_id);

        let mut configurations = vector.configuration_records;
        configurations.sort();
        configurations.dedup();
        let payload = BTreeMap::from([
            (
                "configuration_records",
                serde_json::to_value(configurations).unwrap(),
            ),
            (
                "fingerprint_version",
                serde_json::to_value(FINGERPRINT_VERSION).unwrap(),
            ),
            (
                "identity_records",
                serde_json::to_value(identities).unwrap(),
            ),
        ]);
        assert_eq!(
            sha256_hex(&serde_json::to_vec(&payload).unwrap()),
            vector.configuration_fingerprint
        );
    }
}
