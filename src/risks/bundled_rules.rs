use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::config::{sentra_hash_rule_dir, sentra_home, sentra_ti_rule_dir, sentra_yara_rule_dir};
use crate::{SentraError, SentraResult};

use super::{RuleDirectoryConfig, RuleStore};

const BUNDLED_RULES_ZIP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bundled-rules.zip"));
const VERSION_FILE_NAME: &str = ".bundled-rules-version";

/// Ensure bundled risk rules are released into the default Sentra rule store.
///
/// The function is idempotent. It writes a version marker under `.sentra` and
/// skips re-importing when the embedded rule archive has already been released.
pub fn ensure_bundled_rules(home: impl AsRef<Path>) -> SentraResult<RuleDirectoryConfig> {
    let home = home.as_ref();
    let config = bundled_rule_directory_config(home);
    let version = bundled_rules_version();
    let version_path = bundled_rules_version_file(home);

    if fs::read_to_string(&version_path)
        .map(|content| content.trim() == version)
        .unwrap_or(false)
    {
        return Ok(config);
    }

    if !version_path.exists() && default_rule_files_exist(&config) {
        write_version_file(&version_path, &version)?;
        return Ok(config);
    }

    import_bundled_rules(&config)?;
    write_version_file(&version_path, &version)?;
    Ok(config)
}

/// Return the default rule directories used by bundled Sentra rules.
pub fn bundled_rule_directory_config(home: impl AsRef<Path>) -> RuleDirectoryConfig {
    let home = home.as_ref();
    RuleDirectoryConfig {
        yara: Some(sentra_yara_rule_dir(home)),
        ti: Some(sentra_ti_rule_dir(home)),
        hash: Some(sentra_hash_rule_dir(home)),
    }
}

fn import_bundled_rules(config: &RuleDirectoryConfig) -> SentraResult<()> {
    let tmp = tempfile::Builder::new()
        .prefix("sentra-bundled-rules-")
        .tempdir()
        .map_err(|err| SentraError::io(None::<PathBuf>, err))?;
    let zip_path = tmp.path().join("rules.zip");
    fs::write(&zip_path, BUNDLED_RULES_ZIP)
        .map_err(|err| SentraError::io(Some(zip_path.clone()), err))?;

    let store = RuleStore::new(config.clone());
    store.import(zip_path.to_string_lossy().as_ref())?;
    Ok(())
}

fn write_version_file(path: &Path, version: &str) -> SentraResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| SentraError::io(Some(parent.to_path_buf()), err))?;
    }
    fs::write(path, format!("{version}\n"))
        .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))
}

fn default_rule_files_exist(config: &RuleDirectoryConfig) -> bool {
    [&config.yara, &config.ti, &config.hash]
        .into_iter()
        .filter_map(Option::as_deref)
        .any(contains_file)
}

fn contains_file(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() || (path.is_dir() && contains_file(&path)) {
            return true;
        }
    }
    false
}

fn bundled_rules_version_file(home: &Path) -> PathBuf {
    sentra_home(home).join(VERSION_FILE_NAME)
}

fn bundled_rules_version() -> String {
    let hash = Sha256::digest(BUNDLED_RULES_ZIP);
    format!("{}:{hash:x}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn first_use_imports_bundled_rules_and_writes_version() {
        let dir = tempfile::tempdir().unwrap();

        let config = ensure_bundled_rules(dir.path()).unwrap();

        assert!(bundled_rules_version_file(dir.path()).is_file());
        assert!(config.yara.as_ref().unwrap().is_dir());
        assert!(config.ti.as_ref().unwrap().is_dir());
        assert!(config.hash.as_ref().unwrap().is_dir());
        assert!(contains_file(config.yara.as_ref().unwrap()));
    }

    #[test]
    fn matching_version_skips_reimport() {
        let dir = tempfile::tempdir().unwrap();
        let config = ensure_bundled_rules(dir.path()).unwrap();

        let marker = config
            .yara
            .as_ref()
            .unwrap()
            .join("prompt_injection_generic.yara");
        fs::write(&marker, "local edit").unwrap();

        ensure_bundled_rules(dir.path()).unwrap();

        assert_eq!(fs::read_to_string(marker).unwrap(), "local edit");
    }

    #[test]
    fn existing_manual_rules_are_adopted_on_first_use() {
        let dir = tempfile::tempdir().unwrap();
        let manual = sentra_yara_rule_dir(dir.path()).join("manual.yar");
        fs::create_dir_all(manual.parent().unwrap()).unwrap();
        fs::write(&manual, "rule Manual { condition: true }").unwrap();

        ensure_bundled_rules(dir.path()).unwrap();

        assert_eq!(
            fs::read_to_string(bundled_rules_version_file(dir.path()))
                .unwrap()
                .trim(),
            bundled_rules_version()
        );
        assert!(
            !sentra_yara_rule_dir(dir.path())
                .join("prompt_injection_generic.yara")
                .exists()
        );
    }
}
