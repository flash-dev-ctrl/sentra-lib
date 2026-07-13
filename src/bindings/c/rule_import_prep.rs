use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::utils::extract_zip_to_dir;
use crate::{SentraError, SentraResult};

/// Extract a rules zip archive (the caller guarantees `zip_path` is a valid
/// zip file) into a fresh temporary directory, dropping macOS metadata
/// entries (`__MACOSX/`, `.DS_Store`, `._*`) along the way.
///
/// The returned directory can be handed directly to `RuleStore::import()`,
/// which already supports importing from a directory. This keeps all zip
/// handling in the C-binding layer so `crate::risks` never needs to change.
pub(crate) fn extract_rules_zip(zip_path: &Path) -> SentraResult<tempfile::TempDir> {
    let tmp = tempfile::Builder::new()
        .prefix("sentra-import-")
        .tempdir()
        .map_err(|err| SentraError::io(None::<PathBuf>, err))?;
    extract_zip_to_dir(zip_path, tmp.path())?;
    remove_macos_junk(tmp.path())?;
    Ok(tmp)
}

fn remove_macos_junk(dir: &Path) -> SentraResult<()> {
    let mut junk_dirs = Vec::new();
    let mut junk_files = Vec::new();

    for entry in WalkDir::new(dir).min_depth(1) {
        let entry = entry
            .map_err(|err| SentraError::Message(format!("failed to walk extracted zip: {err}")))?;
        let name = entry.file_name().to_string_lossy();
        if name == "__MACOSX" || name == ".DS_Store" || name.starts_with("._") {
            if entry.file_type().is_dir() {
                junk_dirs.push(entry.path().to_path_buf());
            } else {
                junk_files.push(entry.path().to_path_buf());
            }
        }
    }

    for file in junk_files {
        let _ = fs::remove_file(file);
    }
    for dir in junk_dirs {
        let _ = fs::remove_dir_all(dir);
    }
    Ok(())
}
