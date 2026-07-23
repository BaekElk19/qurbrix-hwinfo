use crate::error::{InventoryError, Result};
use hw_model::{ArtifactMetadata, ScanReport, SnapshotId};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

pub(crate) fn ensure_private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

pub(crate) fn ensure_private_file(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn safe_artifact_path(state_dir: &Path, relative_path: &str) -> Result<PathBuf> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || relative.components().count() != 2
        || relative
            .components()
            .next()
            .and_then(|part| part.as_os_str().to_str())
            != Some("reports")
    {
        return Err(InventoryError::InvalidArtifactPath(relative.to_path_buf()));
    }
    Ok(state_dir.join(relative))
}

pub(crate) fn write_report(
    state_dir: &Path,
    snapshot_id: SnapshotId,
    report: &ScanReport,
) -> Result<ArtifactMetadata> {
    let reports_dir = state_dir.join("reports");
    ensure_private_directory(&reports_dir)?;
    let relative_path = format!("reports/{snapshot_id}.json");
    let final_path = safe_artifact_path(state_dir, &relative_path)?;
    let temp_path = reports_dir.join(format!(
        ".{snapshot_id}.{}.snapshot.tmp",
        std::process::id()
    ));
    let bytes = serde_json::to_vec(report)?;

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temp_path)?;
    let write_result = (|| -> Result<()> {
        file.write_all(&bytes)?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temp_path, &final_path)?;
        File::open(&reports_dir)?.sync_all()?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result?;
    ensure_private_file(&final_path)?;

    Ok(ArtifactMetadata {
        relative_path,
        sha256: hex::encode(Sha256::digest(&bytes)),
        size_bytes: bytes.len() as u64,
        schema_version: report.schema_version.clone(),
    })
}

pub(crate) fn read_report(state_dir: &Path, metadata: &ArtifactMetadata) -> Result<ScanReport> {
    let path = safe_artifact_path(state_dir, &metadata.relative_path)?;
    let file_metadata = fs::symlink_metadata(&path)?;
    if !file_metadata.file_type().is_file() || file_metadata.len() != metadata.size_bytes {
        return Err(InventoryError::ArtifactSizeMismatch);
    }
    let mut bytes = Vec::with_capacity(metadata.size_bytes as usize);
    File::open(path)?.read_to_end(&mut bytes)?;
    if hex::encode(Sha256::digest(&bytes)) != metadata.sha256 {
        return Err(InventoryError::ArtifactHashMismatch);
    }
    let report: ScanReport = serde_json::from_slice(&bytes)?;
    if report.schema_version != metadata.schema_version {
        return Err(InventoryError::ArtifactSchemaMismatch);
    }
    Ok(report)
}

pub(crate) fn remove_report(state_dir: &Path, relative_path: &str) -> Result<()> {
    let path = safe_artifact_path(state_dir, relative_path)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn recover_orphans(state_dir: &Path, known_paths: &[String]) -> Result<u64> {
    let reports_dir = state_dir.join("reports");
    ensure_private_directory(&reports_dir)?;
    let mut removed = 0;
    for entry in fs::read_dir(reports_dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let relative = format!("reports/{name}");
        let is_temp = name.ends_with(".snapshot.tmp");
        let is_orphan_json = name.ends_with(".json") && !known_paths.contains(&relative);
        if is_temp || is_orphan_json {
            fs::remove_file(entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::safe_artifact_path;
    use std::path::Path;

    #[test]
    fn rejects_path_traversal_and_absolute_paths() {
        let state = Path::new("/tmp/state");
        assert!(safe_artifact_path(state, "../secret").is_err());
        assert!(safe_artifact_path(state, "/tmp/report.json").is_err());
        assert!(safe_artifact_path(state, "reports/a/extra.json").is_err());
        assert!(safe_artifact_path(state, "reports/id.json").is_ok());
    }
}
