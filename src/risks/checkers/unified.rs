use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use crate::SentraResult;
use crate::interfaces::{CheckInput, CheckResult, CheckStatus, Checker, Finding};
use crate::risks::checkers::hash::{HASH_CHECKER_ID, HASH_WHITELIST_REASON, HashChecker};
use crate::risks::checkers::llm::LlmChecker;
use crate::risks::checkers::ti::ThreatIntelChecker;
use crate::risks::checkers::yara::{YARA_CHECKER_ID, YaraChecker};
use crate::risks::rule_store::RuleStore;
use crate::risks::types::{RuleLoadSummary, RuleType, ScanOptions};
use crate::utils::{compute_content_hashes, resolve_content_meta};
use futures::StreamExt;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CheckError {
    pub checker: String,
    pub source: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct CheckResultCache {
    entries: HashMap<String, CheckOutput>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CheckOutput {
    pub findings: Vec<Finding>,
    pub errors: Vec<CheckError>,
}

pub struct RiskChecker {
    options: ScanOptions,
    state: Mutex<RiskCheckerState>,
    cache: Mutex<CheckResultCache>,
    cache_fingerprint: String,
    skip_cache: bool,
}

struct RiskCheckerState {
    checkers: Vec<Arc<dyn Checker>>,
    llm_checker: Option<Arc<LlmChecker>>,
    rules: RuleStore,
    loaded_rule_types: BTreeSet<RuleType>,
}

pub(crate) fn ok(checker: &str, findings: Vec<Finding>) -> CheckResult {
    CheckResult {
        checker: checker.to_string(),
        status: CheckStatus::Ok,
        reason: None,
        findings,
    }
}

pub(crate) fn skipped(checker: &str, reason: &str) -> CheckResult {
    CheckResult {
        checker: checker.to_string(),
        status: CheckStatus::Skipped,
        reason: Some(reason.to_string()),
        findings: Vec::new(),
    }
}

impl RiskChecker {
    pub fn new(options: ScanOptions) -> SentraResult<Self> {
        let cache_fingerprint = cache_fingerprint(&options);
        let skip_cache = options
            .cache
            .as_ref()
            .map(|cache| cache.skip_cache)
            .unwrap_or(false);
        let mut state = RiskCheckerState {
            checkers: Vec::new(),
            llm_checker: None,
            rules: RuleStore::new(options.rules.clone().unwrap_or_default()),
            loaded_rule_types: BTreeSet::new(),
        };
        rebuild_checkers(&options, &mut state);
        Ok(Self {
            options: options.clone(),
            state: Mutex::new(state),
            cache: Mutex::new(CheckResultCache::default()),
            cache_fingerprint,
            skip_cache,
        })
    }

    pub fn load_rule(&self, rule_type: RuleType) -> SentraResult<RuleLoadSummary> {
        let mut state = self.lock_state();
        state.rules.refresh_rule(rule_type)?;
        state.loaded_rule_types.insert(rule_type);
        rebuild_checkers(&self.options, &mut state);
        Ok(rule_load_summary(&state.rules))
    }

    pub fn load_rules(&self) -> SentraResult<RuleLoadSummary> {
        let mut state = self.lock_state();
        for rule_type in enabled_rule_types(&self.options) {
            state.rules.refresh_rule(rule_type)?;
            state.loaded_rule_types.insert(rule_type);
        }
        rebuild_checkers(&self.options, &mut state);
        Ok(rule_load_summary(&state.rules))
    }

    pub fn enabled_rule_types(&self) -> Vec<RuleType> {
        enabled_rule_types(&self.options)
    }

    pub fn concurrency(&self) -> usize {
        normalize_concurrency(self.options.concurrency)
    }

    pub async fn scan(&self, inputs: &[CheckInput]) -> SentraResult<CheckOutput> {
        self.ensure_rules_loaded()?;
        let (checkers, llm_checker) = self.checker_snapshot();
        let mut output = CheckOutput::default();
        let mut completed = Vec::new();
        let mut pending = Vec::new();

        for (index, raw) in inputs.iter().enumerate() {
            let input = fill_meta(raw);
            let cache_key = read_sha256(&input).map(|key| self.scoped_cache_key(&key));
            if let Some(cache_key) = &cache_key
                && self.skip_cache
            {
                self.lock_cache().clear_key(cache_key);
            }
            if let Some(cache_key) = &cache_key
                && !self.skip_cache
                && let Some(cached) = self.lock_cache().get(cache_key).cloned()
            {
                let cloned = clone_cached_output(&cached, input.source());
                completed.push((index, cloned));
                continue;
            }

            pending.push((index, input, cache_key));
        }

        let mut stream =
            futures::stream::iter(pending.into_iter().map(|(index, input, cache_key)| {
                let checkers = checkers.clone();
                let llm_checker = llm_checker.clone();
                async move {
                    tokio::spawn(async move {
                        let input_output =
                            scan_one_with(&input, &checkers, llm_checker.as_deref()).await?;
                        Ok::<_, crate::SentraError>((index, cache_key, input_output))
                    })
                    .await
                    .map_err(|err| crate::SentraError::Message(err.to_string()))?
                }
            }))
            .buffer_unordered(self.concurrency());

        while let Some(result) = stream.next().await {
            let (index, cache_key, input_output) = result?;
            if let Some(cache_key) = cache_key {
                self.lock_cache().insert(cache_key, input_output.clone());
            }
            completed.push((index, input_output));
        }

        completed.sort_by_key(|(index, _)| *index);
        for (_, input_output) in completed {
            output.findings.extend(input_output.findings);
            output.errors.extend(input_output.errors);
        }
        Ok(output)
    }

    fn checker_snapshot(&self) -> (Vec<Arc<dyn Checker>>, Option<Arc<LlmChecker>>) {
        let state = self.lock_state();
        (state.checkers.clone(), state.llm_checker.clone())
    }

    fn lock_cache(&self) -> std::sync::MutexGuard<'_, CheckResultCache> {
        self.cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, RiskCheckerState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn ensure_rules_loaded(&self) -> SentraResult<()> {
        let mut state = self.lock_state();
        for rule_type in enabled_rule_types(&self.options) {
            if !state.loaded_rule_types.contains(&rule_type) {
                state.rules.refresh_rule(rule_type)?;
                state.loaded_rule_types.insert(rule_type);
                rebuild_checkers(&self.options, &mut state);
            }
        }
        Ok(())
    }

    fn scoped_cache_key(&self, input_key: &str) -> String {
        format!("{}:{input_key}", self.cache_fingerprint)
    }
}

async fn scan_one_with(
    input: &CheckInput,
    checkers: &[Arc<dyn Checker>],
    llm_checker: Option<&LlmChecker>,
) -> SentraResult<CheckOutput> {
    let mut findings = Vec::new();
    let mut errors = Vec::new();
    let input_cat = input.file_category();
    let has_yara_checker = checkers
        .iter()
        .any(|checker| checker.id() == YARA_CHECKER_ID);
    for checker in checkers {
        if !checker.categories().is_empty()
            && input_cat.is_some()
            && !checker.categories().contains(&input_cat.unwrap())
        {
            continue;
        }
        let result = checker.check(input).await?;
        if result.status == CheckStatus::Error {
            errors.push(CheckError {
                checker: result.checker,
                source: input.source().to_string(),
                reason: result.reason.unwrap_or_default(),
            });
        } else if result.status == CheckStatus::Ok {
            if result.checker == HASH_CHECKER_ID
                && result.reason.as_deref() == Some(HASH_WHITELIST_REASON)
            {
                return Ok(CheckOutput::default());
            }

            let is_hash = result.checker == HASH_CHECKER_ID;
            let is_yara = result.checker == YARA_CHECKER_ID;
            let has_risk = !result.findings.is_empty();

            if is_yara && has_risk {
                let llm_reviewed = apply_llm_review(
                    llm_checker,
                    input,
                    input.source(),
                    &mut findings,
                    &mut errors,
                )
                .await?;
                if llm_reviewed {
                    continue;
                }
            }

            findings.extend(result.findings);

            if is_hash && has_risk {
                break;
            }
        }
    }
    let should_run_llm_directly = !has_yara_checker
        && llm_checker.is_some_and(|llm| {
            llm.categories().is_empty()
                || input_cat.is_none()
                || input_cat.is_some_and(|cat| llm.categories().contains(&cat))
        });
    if should_run_llm_directly {
        apply_llm_review(
            llm_checker,
            input,
            input.source(),
            &mut findings,
            &mut errors,
        )
        .await?;
    }
    Ok(CheckOutput { findings, errors })
}

fn rebuild_checkers(options: &ScanOptions, state: &mut RiskCheckerState) {
    let mut llm_checker = None;
    state.checkers = build_checkers(options, state.rules.clone(), &mut llm_checker);
    state.llm_checker = llm_checker;
}

fn build_checkers(
    options: &ScanOptions,
    rules: RuleStore,
    llm_checker: &mut Option<Arc<LlmChecker>>,
) -> Vec<Arc<dyn Checker>> {
    let config = options.checker.as_ref();
    let llm = options.llm.clone();
    let online_ti = options.online_ti.clone();
    let mut checkers: Vec<Arc<dyn Checker>> = Vec::new();

    // Scan order is intentional: hash can whitelist/blacklist and short-circuit,
    // YARA provides local rule hits, TI is a secondary local/online signal.
    for checker in [
        CheckerKind::Hash,
        CheckerKind::Yara,
        CheckerKind::ThreatIntel,
    ] {
        match checker {
            CheckerKind::Hash if enabled(config.and_then(|config| config.enable_hash)) => {
                checkers.push(Arc::new(HashChecker::new(rules.hash().clone())));
            }
            CheckerKind::Yara if enabled(config.and_then(|config| config.enable_yara)) => {
                checkers.push(Arc::new(YaraChecker::new(rules.compiled_yara().to_vec())));
            }
            CheckerKind::ThreatIntel
                if enabled(config.and_then(|config| config.enable_local_ti)) =>
            {
                checkers.push(Arc::new(ThreatIntelChecker::new_with_online_ti(
                    rules.ti().clone(),
                    if enabled(config.and_then(|config| config.enable_online_ti)) {
                        online_ti.clone()
                    } else {
                        None
                    },
                )));
            }
            _ => {}
        }
    }

    if config
        .and_then(|config| config.enable_llm)
        .unwrap_or(llm.is_some())
    {
        *llm_checker = Some(Arc::new(LlmChecker::new(llm)));
    }
    checkers
}

fn enabled_rule_types(options: &ScanOptions) -> Vec<RuleType> {
    let config = options.checker.as_ref();
    let mut rule_types = Vec::new();
    if enabled(config.and_then(|config| config.enable_hash)) {
        rule_types.push(RuleType::Hash);
    }
    if enabled(config.and_then(|config| config.enable_yara)) {
        rule_types.push(RuleType::Yara);
    }
    if enabled(config.and_then(|config| config.enable_local_ti)) {
        rule_types.push(RuleType::ThreatIntel);
    }
    rule_types
}

fn rule_load_summary(rules: &RuleStore) -> RuleLoadSummary {
    RuleLoadSummary {
        yara: rules.yara().len(),
        ti_ips: rules.ti().malicious_ips.len(),
        ti_domains: rules.ti().malicious_domains.len(),
        hash_blacklist: rules.hash().blacklist.len(),
        hash_whitelist: rules.hash().whitelist.len(),
    }
}

#[derive(Clone, Copy)]
enum CheckerKind {
    Hash,
    Yara,
    ThreatIntel,
}

fn enabled(value: Option<bool>) -> bool {
    value.unwrap_or(true)
}

fn normalize_concurrency(value: Option<usize>) -> usize {
    value.filter(|value| *value > 0).unwrap_or(4)
}

async fn apply_llm_review(
    llm: Option<&LlmChecker>,
    input: &CheckInput,
    source: &str,
    findings: &mut Vec<Finding>,
    errors: &mut Vec<CheckError>,
) -> SentraResult<bool> {
    let Some(llm) = llm else {
        return Ok(false);
    };

    let result = llm.check(input).await?;
    if result.status == CheckStatus::Error {
        errors.push(CheckError {
            checker: result.checker,
            source: source.to_string(),
            reason: result.reason.unwrap_or_default(),
        });
        return Ok(false);
    } else if result.status == CheckStatus::Ok {
        findings.extend(result.findings);
        return Ok(true);
    }
    Ok(false)
}

impl CheckResultCache {
    fn get(&self, key: &str) -> Option<&CheckOutput> {
        self.entries.get(key)
    }

    fn insert(&mut self, key: String, value: CheckOutput) {
        self.entries.insert(key, value);
    }

    fn clear_key(&mut self, key: &str) {
        self.entries.remove(key);
    }
}

fn cache_fingerprint(options: &ScanOptions) -> String {
    let value = serde_json::json!({
        "checker": options.checker,
        "llm": options.llm,
        "rules": options.rules,
        "onlineTi": options.online_ti,
    });
    compute_content_hashes(value.to_string().as_bytes())
        .sha256
        .chars()
        .take(16)
        .collect()
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use std::fs;

    #[test]
    fn memory_cache_round_trips_check_outputs() {
        let mut cache = CheckResultCache::default();
        cache.insert(
            "abc".to_string(),
            CheckOutput {
                findings: vec![crate::interfaces::Finding::new(
                    "risk",
                    "test-checker",
                    crate::interfaces::RiskSeverity::High,
                    crate::interfaces::RiskCategory::MaliciousExecution,
                    "SKILL.md",
                    "Risk",
                    "Risk description",
                    "Fix it",
                )],
                errors: Vec::new(),
            },
        );

        assert_eq!(cache.get("abc").unwrap().findings[0].title, "Risk");
    }

    #[test]
    fn clearing_key_allows_new_scan_result_to_replace_old_cache() {
        let mut cache = CheckResultCache::default();
        cache.insert("abc".to_string(), CheckOutput::default());

        cache.clear_key("abc");
        cache.insert(
            "abc".to_string(),
            CheckOutput {
                findings: Vec::new(),
                errors: vec![CheckError {
                    checker: "test".to_string(),
                    source: "SKILL.md".to_string(),
                    reason: "fresh".to_string(),
                }],
            },
        );

        assert_eq!(cache.get("abc").unwrap().errors[0].reason, "fresh");
    }

    #[test]
    fn load_rules_loads_all_rule_types_and_reports_summary() {
        let (_root, rules) = rule_fixture();

        let checker = RiskChecker::new(ScanOptions {
            rules: Some(rules),
            ..ScanOptions::default()
        })
        .unwrap();

        let summary = checker.load_rules().unwrap();

        assert_eq!(summary.yara, 1);
        assert_eq!(summary.ti_ips, 1);
        assert_eq!(summary.ti_domains, 1);
        assert_eq!(summary.hash_blacklist, 1);
        assert_eq!(
            checker.lock_state().loaded_rule_types,
            RuleType::ALL.into_iter().collect()
        );
    }

    #[test]
    fn load_rules_only_loads_enabled_rule_types() {
        let (_root, rules) = rule_fixture();

        let checker = RiskChecker::new(ScanOptions {
            rules: Some(rules),
            checker: Some(crate::risks::types::CheckerConfig {
                enable_hash: Some(true),
                enable_yara: Some(false),
                enable_local_ti: Some(false),
                ..Default::default()
            }),
            ..ScanOptions::default()
        })
        .unwrap();

        let summary = checker.load_rules().unwrap();

        assert_eq!(summary.yara, 0);
        assert_eq!(summary.ti_ips, 0);
        assert_eq!(summary.ti_domains, 0);
        assert_eq!(summary.hash_blacklist, 1);
        assert_eq!(checker.enabled_rule_types(), vec![RuleType::Hash]);
        assert_eq!(
            checker.lock_state().loaded_rule_types,
            [RuleType::Hash].into_iter().collect()
        );
    }

    fn rule_fixture() -> (tempfile::TempDir, crate::risks::types::RuleDirectoryConfig) {
        let root = tempfile::tempdir().unwrap();
        let yara_dir = root.path().join("yara");
        let ti_dir = root.path().join("ti");
        let hash_dir = root.path().join("hash");
        fs::create_dir_all(&yara_dir).unwrap();
        fs::create_dir_all(&ti_dir).unwrap();
        fs::create_dir_all(&hash_dir).unwrap();
        fs::write(
            yara_dir.join("demo.yar"),
            r#"
rule DemoRule {
    strings:
        $marker = "demo-marker"
    condition:
        $marker
}
"#,
        )
        .unwrap();
        fs::write(ti_dir.join("feed.txt"), "203.0.113.8\nbad.example.com\n").unwrap();
        fs::write(
            hash_dir.join("black.txt"),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
        )
        .unwrap();

        (
            root,
            crate::risks::types::RuleDirectoryConfig {
                yara: Some(yara_dir),
                ti: Some(ti_dir),
                hash: Some(hash_dir),
            },
        )
    }
}

fn fill_meta(input: &CheckInput) -> CheckInput {
    let CheckInput::Content(content) = input else {
        return input.clone();
    };
    if content.file_cat.is_some() && content.file_ext.is_some() {
        return input.clone();
    }
    let (cat, ext) = resolve_content_meta(
        &content.source,
        &content.content,
        Some((content.file_cat, content.file_ext)),
    );
    let mut next = content.clone();
    next.file_cat = Some(cat);
    next.file_ext = Some(ext);
    CheckInput::Content(next)
}

fn read_sha256(input: &CheckInput) -> Option<String> {
    let CheckInput::Content(content) = input else {
        return None;
    };
    content
        .other
        .get("hashes")
        .and_then(|value| value.get("sha256"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_ascii_lowercase())
}

fn clone_cached_output(cached: &CheckOutput, source: &str) -> CheckOutput {
    CheckOutput {
        findings: cached
            .findings
            .iter()
            .map(|finding| {
                let mut cloned = finding.clone();
                cloned.file = source.to_string();
                if finding.file != source {
                    cloned.id = format!("{}-{}", finding.id, source_suffix(source));
                }
                cloned
            })
            .collect(),
        errors: cached
            .errors
            .iter()
            .map(|error| CheckError {
                source: source.to_string(),
                ..error.clone()
            })
            .collect(),
    }
}

fn source_suffix(source: &str) -> String {
    compute_content_hashes(source.as_bytes())
        .sha256
        .chars()
        .take(8)
        .collect()
}
