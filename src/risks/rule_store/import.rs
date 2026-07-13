use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::utils::{download_url_to_file, extract_zip_to_dir, url_file_name};
use crate::{SentraError, SentraResult};

use super::detect::detect_rule_file_type;
use super::util::{
    default_hash_dir, default_ti_dir, default_yara_dir, ensure_hash_name, ensure_ti_name,
    ensure_yara_name, is_zip_file, resolve_dir,
};
use super::{ImportResult, RuleFileType, RuleStore};

impl RuleStore {
    /// Import a rule source into the configured rule directories.
    ///
    /// Supports `http://`/`https://` URLs and local paths. Zips are extracted
    /// and each entry is classified and dispatched to `import_yara`,
    /// `import_ti`, or `import_hash`. Directories are scanned recursively.
    pub fn import(&self, source: &str) -> SentraResult<ImportResult> {
        if source.starts_with("http://") || source.starts_with("https://") {
            let tmp = tempfile::Builder::new()
                .prefix("sentra-import-")
                .tempdir()
                .map_err(|err| SentraError::io(None::<PathBuf>, err))?;
            let dest = tmp.path().join(url_file_name(source));
            download_url_to_file(source, &dest)?;
            self.import_path(&dest)
        } else {
            self.import_path(Path::new(source))
        }
    }

    /// Write a YARA rule file into the configured yara directory.
    pub fn import_yara(&self, name: &str, content: &str) -> SentraResult<()> {
        let dir = self.yara_dir()?;
        fs::create_dir_all(&dir).map_err(|err| SentraError::io(Some(dir.clone()), err))?;
        let dest = dir.join(ensure_yara_name(name));
        fs::write(&dest, content).map_err(|err| SentraError::io(Some(dest), err))
    }

    /// Write a threat-intel feed into the configured ti directory.
    pub fn import_ti(&self, name: &str, content: &str) -> SentraResult<()> {
        let dir = self.ti_dir()?;
        fs::create_dir_all(&dir).map_err(|err| SentraError::io(Some(dir.clone()), err))?;
        let dest = dir.join(ensure_ti_name(name));
        fs::write(&dest, content).map_err(|err| SentraError::io(Some(dest), err))
    }

    /// Write a hash list into the configured hash directory.
    pub fn import_hash(&self, name: &str, content: &str) -> SentraResult<()> {
        let dir = self.hash_dir()?;
        fs::create_dir_all(&dir).map_err(|err| SentraError::io(Some(dir.clone()), err))?;
        let dest = dir.join(ensure_hash_name(name));
        fs::write(&dest, content).map_err(|err| SentraError::io(Some(dest), err))
    }

    fn yara_dir(&self) -> SentraResult<PathBuf> {
        resolve_dir(self.config.yara.clone(), default_yara_dir, "yara")
    }

    fn ti_dir(&self) -> SentraResult<PathBuf> {
        resolve_dir(self.config.ti.clone(), default_ti_dir, "ti")
    }

    fn hash_dir(&self) -> SentraResult<PathBuf> {
        resolve_dir(self.config.hash.clone(), default_hash_dir, "hash")
    }

    fn import_path(&self, path: &Path) -> SentraResult<ImportResult> {
        if !path.exists() {
            return Err(SentraError::Message(format!(
                "path not found: {}",
                path.display()
            )));
        }
        if path.is_dir() {
            self.import_dir(path)
        } else if is_zip_file(path) {
            self.import_zip(path)
        } else {
            let content = fs::read_to_string(path)
                .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("import");
            let mut result = ImportResult::default();
            match self.import_file(&content, name, path)? {
                RuleFileType::Yara => result.yara = 1,
                RuleFileType::Ti => result.ti = 1,
                RuleFileType::Hash => result.hash = 1,
                RuleFileType::Unknown => result.skipped = 1,
            }
            Ok(result)
        }
    }

    fn import_file(&self, content: &str, name: &str, path: &Path) -> SentraResult<RuleFileType> {
        let rule_type = detect_rule_file_type(path, content);
        match rule_type {
            RuleFileType::Yara => self.import_yara(name, content)?,
            RuleFileType::Ti => self.import_ti(name, content)?,
            RuleFileType::Hash => self.import_hash(name, content)?,
            RuleFileType::Unknown => {}
        }
        Ok(rule_type)
    }

    fn import_zip(&self, zip_path: &Path) -> SentraResult<ImportResult> {
        let mut result = ImportResult::default();
        let tmp = tempfile::Builder::new()
            .prefix("sentra-zip-")
            .tempdir()
            .map_err(|err| SentraError::io(None::<PathBuf>, err))?;
        extract_zip_to_dir(zip_path, tmp.path())?;
        for entry in WalkDir::new(tmp.path()).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let relative = path
                .strip_prefix(tmp.path())
                .unwrap_or(path)
                .to_string_lossy();
            let content = fs::read_to_string(path)
                .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))?;
            match self.import_file(&content, &relative, path)? {
                RuleFileType::Yara => result.yara += 1,
                RuleFileType::Ti => result.ti += 1,
                RuleFileType::Hash => result.hash += 1,
                RuleFileType::Unknown => result.skipped += 1,
            }
        }
        Ok(result)
    }

    fn import_dir(&self, dir: &Path) -> SentraResult<ImportResult> {
        let mut result = ImportResult::default();
        for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(dir).unwrap_or(path).to_string_lossy();
            if is_zip_file(path) {
                let sub = self.import_zip(path)?;
                result.yara += sub.yara;
                result.ti += sub.ti;
                result.hash += sub.hash;
                result.skipped += sub.skipped;
            } else {
                let content = fs::read_to_string(path)
                    .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))?;
                match self.import_file(&content, &relative, path)? {
                    RuleFileType::Yara => result.yara += 1,
                    RuleFileType::Ti => result.ti += 1,
                    RuleFileType::Hash => result.hash += 1,
                    RuleFileType::Unknown => result.skipped += 1,
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_local_yara_file() {
        let src_dir = tempfile::tempdir().unwrap();
        let yara_dir = tempfile::tempdir().unwrap();
        let file_path = src_dir.path().join("rules.yar");
        fs::write(&file_path, "rule foo { strings: $a = \"x\" }").unwrap();

        let store = RuleStore::new(crate::risks::types::RuleDirectoryConfig {
            yara: Some(yara_dir.path().to_path_buf()),
            ..Default::default()
        });
        let result = store.import(file_path.to_str().unwrap()).unwrap();
        assert_eq!(result.yara, 1);
        assert_eq!(result.skipped, 0);
        assert!(yara_dir.path().join("rules.yar").exists());
    }
}
