use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use sha2::{Digest, Sha256};

use crate::risks::types::{HashRuleDef, RuleDirectoryConfig, RuleType, TiRuleDef, YaraRuleDef};
use crate::{SentraError, SentraResult};

#[derive(Debug, Clone)]
pub struct RuleStore {
    pub(crate) config: RuleDirectoryConfig,
    pub(crate) yara: Vec<YaraRuleDef>,
    pub(crate) compiled_yara: Vec<CompiledYaraRule>,
    yara_fingerprint: Option<YaraRuleSetFingerprint>,
    yara_cache_root: Option<PathBuf>,
    pub(crate) ti: TiRuleDef,
    pub(crate) hash: HashRuleDef,
}

#[derive(Debug, Clone)]
pub struct CompiledYaraRule {
    pub source: std::path::PathBuf,
    pub rules: Arc<yara_x::Rules>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct YaraRuleSetFingerprint {
    files: Vec<YaraRuleFileFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct YaraRuleFileFingerprint {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq)]
pub enum RuleFileType {
    #[default]
    Unknown,
    Yara,
    Ti,
    Hash,
}

#[derive(Debug, Clone, Default)]
pub struct ImportResult {
    pub yara: usize,
    pub ti: usize,
    pub hash: usize,
    pub skipped: usize,
}

impl RuleStore {
    pub fn new(config: RuleDirectoryConfig) -> Self {
        Self {
            config,
            yara: Vec::new(),
            compiled_yara: Vec::new(),
            yara_fingerprint: None,
            yara_cache_root: None,
            ti: TiRuleDef::default(),
            hash: HashRuleDef::default(),
        }
    }

    #[cfg(test)]
    fn with_yara_cache_root(mut self, cache_root: PathBuf) -> Self {
        self.yara_cache_root = Some(cache_root);
        self
    }

    pub fn refresh(&mut self) -> SentraResult<()> {
        self.refresh_yara()?;
        self.ti = self.load_ti();
        self.hash = self.load_hash();
        Ok(())
    }

    pub fn refresh_rule(&mut self, rule_type: RuleType) -> SentraResult<()> {
        match rule_type {
            RuleType::Yara => {
                self.refresh_yara()?;
            }
            RuleType::ThreatIntel => {
                self.ti = self.load_ti();
            }
            RuleType::Hash => {
                self.hash = self.load_hash();
            }
        }
        Ok(())
    }

    pub fn yara(&self) -> &[YaraRuleDef] {
        &self.yara
    }

    pub fn compiled_yara(&self) -> &[CompiledYaraRule] {
        &self.compiled_yara
    }

    pub fn ti(&self) -> &TiRuleDef {
        &self.ti
    }

    pub fn hash(&self) -> &HashRuleDef {
        &self.hash
    }

    pub fn hash_mut(&mut self) -> &mut HashRuleDef {
        &mut self.hash
    }

    fn refresh_yara(&mut self) -> SentraResult<()> {
        let paths = self.yara_rule_paths();
        let fingerprint = yara_rule_set_fingerprint(&paths)?;
        if self.yara_fingerprint.as_ref() == Some(&fingerprint) {
            return Ok(());
        }

        let yara = Self::load_yara_paths(&paths)?;
        let compiled_yara = self.load_or_compile_yara_rules(&yara, &fingerprint)?;
        self.yara = yara;
        self.compiled_yara = compiled_yara;
        self.yara_fingerprint = Some(fingerprint);
        Ok(())
    }

    fn yara_rule_paths(&self) -> Vec<PathBuf> {
        let Some(dir) = &self.config.yara else {
            return Vec::new();
        };
        if !dir.is_dir() {
            return Vec::new();
        }
        collect_files(dir)
            .into_iter()
            .filter(|path| {
                let ext = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or_default();
                ext == "yar" || ext == "yara"
            })
            .collect()
    }

    fn load_yara_paths(paths: &[PathBuf]) -> SentraResult<Vec<YaraRuleDef>> {
        paths
            .par_iter()
            .map(|path| {
                let content = fs::read_to_string(path)
                    .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))?;
                let name = path
                    .file_stem()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default();
                Ok(YaraRuleDef {
                    name,
                    source: path.to_path_buf(),
                    content,
                })
            })
            .collect::<SentraResult<Vec<_>>>()
    }

    fn compile_yara_rules(&self, yara: &[YaraRuleDef]) -> SentraResult<Vec<CompiledYaraRule>> {
        if yara.is_empty() {
            return Ok(Vec::new());
        }

        let mut compiler = yara_x::Compiler::new();
        for (index, rule) in yara.iter().enumerate() {
            compiler.new_namespace(&format!("rule_{index}"));
            compiler
                .add_source(
                    yara_x::SourceCode::from(rule.content.as_str())
                        .with_origin(rule.source.to_string_lossy()),
                )
                .map_err(|err| {
                    SentraError::Message(format!(
                        "failed to compile YARA rule {}: {err}",
                        rule.source.display()
                    ))
                })?;
        }

        Ok(vec![CompiledYaraRule {
            source: self
                .config
                .yara
                .clone()
                .unwrap_or_else(|| PathBuf::from("yara")),
            rules: Arc::new(compiler.build()),
        }])
    }

    fn load_or_compile_yara_rules(
        &self,
        yara: &[YaraRuleDef],
        fingerprint: &YaraRuleSetFingerprint,
    ) -> SentraResult<Vec<CompiledYaraRule>> {
        if yara.is_empty() {
            return Ok(Vec::new());
        }

        let source = self
            .config
            .yara
            .clone()
            .unwrap_or_else(|| PathBuf::from("yara"));
        if let Some(rules) = read_cached_yara_rules_in(self.yara_cache_root.as_deref(), fingerprint)
        {
            return Ok(vec![CompiledYaraRule {
                source,
                rules: Arc::new(rules),
            }]);
        }

        let compiled_yara = self.compile_yara_rules(yara)?;
        write_cached_yara_rules_in(self.yara_cache_root.as_deref(), fingerprint, &compiled_yara);
        Ok(compiled_yara)
    }

    fn load_ti(&self) -> TiRuleDef {
        let Some(dir) = &self.config.ti else {
            return TiRuleDef::default();
        };
        if !dir.is_dir() {
            return TiRuleDef::default();
        }
        collect_files(dir)
            .par_iter()
            .filter_map(|path| {
                let file = fs::File::open(path).ok()?;
                let size = file
                    .metadata()
                    .ok()
                    .map(|metadata| metadata.len())
                    .unwrap_or(0);
                Some((file, size))
            })
            .map(|(file, size)| {
                let mut data = ti_rule_def_with_capacity(size);
                for line in BufReader::with_capacity(64 * 1024, file)
                    .lines()
                    .map_while(Result::ok)
                {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    for indicator in ti_indicators(line) {
                        match indicator {
                            TiIndicator::Ip(value) => {
                                data.malicious_ips.insert(value);
                            }
                            TiIndicator::Domain(value) => {
                                data.malicious_domains.insert(value);
                            }
                        }
                    }
                }
                data
            })
            .reduce(TiRuleDef::default, merge_ti_rules)
    }

    fn load_hash(&self) -> HashRuleDef {
        let Some(dir) = &self.config.hash else {
            return HashRuleDef::default();
        };
        if !dir.is_dir() {
            return HashRuleDef::default();
        }
        collect_files(dir)
            .par_iter()
            .map(|path| {
                let content = fs::read_to_string(path).unwrap_or_default();
                let is_white = path
                    .file_name()
                    .map(|name| name.to_string_lossy().contains("white"))
                    .unwrap_or(false);
                let mut data = HashRuleDef::default();
                for line in content
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty() && !line.starts_with('#'))
                {
                    if is_white {
                        data.whitelist.insert(line.to_ascii_lowercase());
                    } else {
                        data.blacklist.insert(line.to_ascii_lowercase());
                    }
                }
                data
            })
            .reduce(HashRuleDef::default, merge_hash_rules)
    }
}

fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn yara_rule_set_fingerprint(paths: &[PathBuf]) -> SentraResult<YaraRuleSetFingerprint> {
    let files = paths
        .iter()
        .map(|path| {
            let metadata =
                fs::metadata(path).map_err(|err| SentraError::io(Some(path.to_path_buf()), err))?;
            Ok(YaraRuleFileFingerprint {
                path: path.clone(),
                len: metadata.len(),
                modified: metadata.modified().ok(),
            })
        })
        .collect::<SentraResult<Vec<_>>>()?;
    Ok(YaraRuleSetFingerprint { files })
}

fn read_cached_yara_rules_in(
    cache_root: Option<&Path>,
    fingerprint: &YaraRuleSetFingerprint,
) -> Option<yara_x::Rules> {
    let path = yara_rules_cache_path_in(cache_root, fingerprint)?;
    let bytes = fs::read(path).ok()?;
    yara_x::Rules::deserialize(bytes).ok()
}

fn write_cached_yara_rules_in(
    cache_root: Option<&Path>,
    fingerprint: &YaraRuleSetFingerprint,
    compiled_yara: &[CompiledYaraRule],
) {
    let Some(rule) = compiled_yara.first() else {
        return;
    };
    let Some(path) = yara_rules_cache_path_in(cache_root, fingerprint) else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    let Ok(bytes) = rule.rules.serialize() else {
        return;
    };
    if fs::create_dir_all(parent).is_ok() {
        let _ = fs::write(path, bytes);
    }
}

fn yara_rules_cache_path_in(
    cache_root: Option<&Path>,
    fingerprint: &YaraRuleSetFingerprint,
) -> Option<PathBuf> {
    let cache_root = match cache_root {
        Some(path) => path.to_path_buf(),
        None => home::home_dir()?.join(".sentra").join("cache"),
    };
    Some(
        cache_root
            .join("yara")
            .join(format!("{}.bin", yara_rule_set_cache_key(fingerprint))),
    )
}

fn yara_rule_set_cache_key(fingerprint: &YaraRuleSetFingerprint) -> String {
    let mut hasher = Sha256::new();
    for file in &fingerprint.files {
        hasher.update(file.path.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(file.len.to_le_bytes());
        hasher.update([0]);
        let modified = file
            .modified
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
        if let Some(modified) = modified {
            hasher.update(modified.as_secs().to_le_bytes());
            hasher.update(modified.subsec_nanos().to_le_bytes());
        }
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn merge_ti_rules(mut left: TiRuleDef, right: TiRuleDef) -> TiRuleDef {
    left.malicious_ips.extend(right.malicious_ips);
    left.malicious_domains.extend(right.malicious_domains);
    left
}

fn merge_hash_rules(mut left: HashRuleDef, right: HashRuleDef) -> HashRuleDef {
    left.blacklist.extend(right.blacklist);
    left.whitelist.extend(right.whitelist);
    left
}

fn ti_rule_def_with_capacity(file_size: u64) -> TiRuleDef {
    let estimated_indicators = (file_size as usize / 16).clamp(16, 1_000_000);
    TiRuleDef {
        malicious_ips: HashSet::with_capacity(estimated_indicators),
        malicious_domains: HashSet::with_capacity(estimated_indicators / 8),
    }
}

enum TiIndicator {
    Ip(String),
    Domain(String),
}

fn ti_indicators(line: &str) -> impl Iterator<Item = TiIndicator> + '_ {
    line.split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';'))
        .map(|item| {
            item.trim_matches(|ch: char| {
                matches!(
                    ch,
                    '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '`'
                )
            })
        })
        .filter(|item| !item.is_empty())
        .filter_map(ti_indicator)
}

fn ti_indicator(item: &str) -> Option<TiIndicator> {
    if is_ipv4_indicator(item) {
        return Some(TiIndicator::Ip(item.to_string()));
    }

    let domain = item.trim_end_matches('.');
    if is_domain_indicator(domain) {
        Some(TiIndicator::Domain(domain.to_ascii_lowercase()))
    } else {
        None
    }
}

fn is_ipv4_indicator(item: &str) -> bool {
    let mut parts = item.split('.');
    let Some(a) = parts.next() else {
        return false;
    };
    let Some(b) = parts.next() else {
        return false;
    };
    let Some(c) = parts.next() else {
        return false;
    };
    let Some(d) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && [a, b, c, d].iter().all(|part| {
            !part.is_empty()
                && part.chars().all(|ch| ch.is_ascii_digit())
                && part.parse::<u8>().is_ok()
        })
}

fn is_domain_indicator(item: &str) -> bool {
    let mut labels = item.split('.').peekable();
    if labels.peek().is_none() {
        return false;
    }

    let mut count = 0;
    let mut last = "";
    for label in labels {
        count += 1;
        last = label;
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        {
            return false;
        }
    }

    count >= 2 && last.len() >= 2 && last.chars().all(|ch| ch.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use super::*;

    const FIRST_YARA_RULE: &str = r#"
rule FirstMarker {
    strings:
        $marker = "first-marker"
    condition:
        $marker
}
"#;

    const SECOND_YARA_RULE: &str = r#"
rule SecondMarker {
    strings:
        $marker = "second-marker-with-longer-content"
    condition:
        $marker
}
"#;

    #[test]
    fn refresh_yara_reuses_compiled_rules_when_files_are_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("marker.yar"), FIRST_YARA_RULE).unwrap();
        let mut store = RuleStore::new(RuleDirectoryConfig {
            yara: Some(dir.path().to_path_buf()),
            ..Default::default()
        });

        store.refresh_rule(RuleType::Yara).unwrap();
        let first_rules = Arc::clone(&store.compiled_yara[0].rules);

        store.refresh_rule(RuleType::Yara).unwrap();
        let second_rules = Arc::clone(&store.compiled_yara[0].rules);

        assert!(Arc::ptr_eq(&first_rules, &second_rules));
    }

    #[test]
    fn refresh_yara_recompiles_rules_when_file_content_changes() {
        let dir = tempfile::tempdir().unwrap();
        let rule_path = dir.path().join("marker.yar");
        fs::write(&rule_path, FIRST_YARA_RULE).unwrap();
        let mut store = RuleStore::new(RuleDirectoryConfig {
            yara: Some(dir.path().to_path_buf()),
            ..Default::default()
        });

        store.refresh_rule(RuleType::Yara).unwrap();
        let first_rules = Arc::clone(&store.compiled_yara[0].rules);

        fs::write(&rule_path, SECOND_YARA_RULE).unwrap();
        store.refresh_rule(RuleType::Yara).unwrap();
        let second_rules = Arc::clone(&store.compiled_yara[0].rules);

        assert!(!Arc::ptr_eq(&first_rules, &second_rules));
        assert!(store.yara[0].content.contains("second-marker"));
    }

    #[test]
    fn refresh_yara_writes_serialized_rule_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache_dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("marker.yar"), FIRST_YARA_RULE).unwrap();
        let mut store = RuleStore::new(RuleDirectoryConfig {
            yara: Some(dir.path().to_path_buf()),
            ..Default::default()
        })
        .with_yara_cache_root(cache_dir.path().to_path_buf());

        store.refresh_rule(RuleType::Yara).unwrap();
        let fingerprint = store.yara_fingerprint.as_ref().unwrap();
        let cache_path = yara_rules_cache_path_in(Some(cache_dir.path()), fingerprint).unwrap();

        assert!(cache_path.is_file());
        assert!(read_cached_yara_rules_in(Some(cache_dir.path()), fingerprint).is_some());
    }
}
