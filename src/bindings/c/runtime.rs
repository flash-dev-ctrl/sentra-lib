use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use crate::config::{
    SENTRA_CONFIG_FILE_NAME, SENTRA_HASH_RULE_DIR_NAME, SENTRA_HOME_DIR_NAME,
    SENTRA_TI_RULE_DIR_NAME, SENTRA_YARA_RULE_DIR_NAME, sentra_config_file, sentra_hash_rule_dir,
    sentra_ti_rule_dir, sentra_yara_rule_dir,
};
use crate::risks::RuleDirectoryConfig;
use crate::{SentraError, SentraResult};

static RUNTIME_PATHS: OnceLock<Mutex<Option<SentraRuntimePaths>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SentraRuntimePaths {
    pub workspace_root: PathBuf,
    pub sentra_home: PathBuf,
    pub config_file: PathBuf,
    pub rule_dirs: SentraRuleDirs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SentraRuleDirs {
    pub hash: PathBuf,
    pub yara: PathBuf,
    pub ti: PathBuf,
}

pub fn initialize_workspace(workspace_root: impl AsRef<Path>) -> SentraResult<SentraRuntimePaths> {
    let workspace_root = workspace_root.as_ref();
    if workspace_root.as_os_str().is_empty() {
        return Err(SentraError::Message(
            "workspace_root must not be empty".to_string(),
        ));
    }

    let paths = SentraRuntimePaths::new(workspace_root);
    fs::create_dir_all(&paths.workspace_root)
        .map_err(|err| SentraError::io(Some(paths.workspace_root.clone()), err))?;
    fs::create_dir_all(&paths.sentra_home)
        .map_err(|err| SentraError::io(Some(paths.sentra_home.clone()), err))?;
    fs::create_dir_all(&paths.rule_dirs.hash)
        .map_err(|err| SentraError::io(Some(paths.rule_dirs.hash.clone()), err))?;
    fs::create_dir_all(&paths.rule_dirs.yara)
        .map_err(|err| SentraError::io(Some(paths.rule_dirs.yara.clone()), err))?;
    fs::create_dir_all(&paths.rule_dirs.ti)
        .map_err(|err| SentraError::io(Some(paths.rule_dirs.ti.clone()), err))?;
    ensure_config_file(&paths.config_file)?;

    *runtime_paths_guard() = Some(paths.clone());
    Ok(paths)
}

pub fn active_config_file(user_home: impl AsRef<Path>) -> PathBuf {
    initialized_runtime_paths()
        .map(|paths| paths.config_file)
        .unwrap_or_else(|| sentra_config_file(user_home))
}

pub fn active_hash_rule_dir(user_home: impl AsRef<Path>) -> PathBuf {
    initialized_runtime_paths()
        .map(|paths| paths.rule_dirs.hash)
        .unwrap_or_else(|| sentra_hash_rule_dir(user_home))
}

pub fn active_yara_rule_dir(user_home: impl AsRef<Path>) -> PathBuf {
    initialized_runtime_paths()
        .map(|paths| paths.rule_dirs.yara)
        .unwrap_or_else(|| sentra_yara_rule_dir(user_home))
}

pub fn active_ti_rule_dir(user_home: impl AsRef<Path>) -> PathBuf {
    initialized_runtime_paths()
        .map(|paths| paths.rule_dirs.ti)
        .unwrap_or_else(|| sentra_ti_rule_dir(user_home))
}

pub fn initialized_rule_directory_config() -> Option<RuleDirectoryConfig> {
    initialized_runtime_paths().map(|paths| RuleDirectoryConfig {
        hash: Some(paths.rule_dirs.hash),
        yara: Some(paths.rule_dirs.yara),
        ti: Some(paths.rule_dirs.ti),
    })
}

fn initialized_runtime_paths() -> Option<SentraRuntimePaths> {
    runtime_paths_guard().clone()
}

impl SentraRuntimePaths {
    fn new(workspace_root: &Path) -> Self {
        let workspace_root = workspace_root.to_path_buf();
        let sentra_home = workspace_root.join(SENTRA_HOME_DIR_NAME);
        Self {
            config_file: sentra_home.join(SENTRA_CONFIG_FILE_NAME),
            rule_dirs: SentraRuleDirs {
                hash: sentra_home.join(SENTRA_HASH_RULE_DIR_NAME),
                yara: sentra_home.join(SENTRA_YARA_RULE_DIR_NAME),
                ti: sentra_home.join(SENTRA_TI_RULE_DIR_NAME),
            },
            sentra_home,
            workspace_root,
        }
    }
}

fn ensure_config_file(path: &Path) -> SentraResult<()> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => Ok(()),
        Ok(_) => Err(SentraError::Message(format!(
            "config path is not a file: {}",
            path.display()
        ))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            fs::write(path, "{\n}\n").map_err(|err| SentraError::io(Some(path.to_path_buf()), err))
        }
        Err(err) => Err(SentraError::io(Some(path.to_path_buf()), err)),
    }
}

fn runtime_paths_guard() -> std::sync::MutexGuard<'static, Option<SentraRuntimePaths>> {
    RUNTIME_PATHS
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_paths_fall_back_to_user_home_without_initialization() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("user-home");

        assert_eq!(
            active_config_file(&home),
            home.join(".sentra").join("config.json")
        );
        assert_eq!(
            active_hash_rule_dir(&home),
            home.join(".sentra").join("hash")
        );
        assert_eq!(
            active_yara_rule_dir(&home),
            home.join(".sentra").join("yara")
        );
        assert_eq!(active_ti_rule_dir(&home), home.join(".sentra").join("ti"));
    }
}
