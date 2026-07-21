use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::agents::discover_agents;
use crate::interfaces::{AssetType, SkillData};
use crate::risks::types::{CheckerConfig, LlmConfig, RuleDirectoryConfig, ScanOptions};
use crate::risks::{RiskAsset, RiskScanner, ScanReport};
use crate::{SentraError, SentraResult};

use super::runtime;
use super::types::{ScanRequest, ScannerSelection, UnifiedAsset};

const ALL_ASSET_TYPES: [AssetType; 6] = [
    AssetType::Skill,
    AssetType::Mcp,
    AssetType::Provider,
    AssetType::Memory,
    AssetType::Cron,
    AssetType::Plugin,
];

/// Collect all assets (skill, mcp, provider, memory, cron, plugin) from all discovered agents.
pub fn collect_all_assets(home: Option<&Path>) -> SentraResult<Vec<UnifiedAsset>> {
    let home = home.map(Path::to_path_buf);
    block_on(async move { collect_all_assets_async(home.as_deref()).await })
}

/// Collect all assets (skill, mcp, provider, memory, cron, plugin) from all discovered agents.
pub async fn collect_all_assets_async(home: Option<&Path>) -> SentraResult<Vec<UnifiedAsset>> {
    let home = resolve_home(home)?;
    let user_name = home
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let mut assets = Vec::new();
    for agent in discover_agents(&home) {
        let agent_name = agent.name().to_string();
        let agent_title = agent.title().to_string();
        let agent_home = agent.home().to_path_buf();
        for asset_type in &ALL_ASSET_TYPES {
            for asset in agent.get_assets(*asset_type)? {
                let data = asset.data_async().await?;
                if is_empty_data(&data) {
                    continue;
                }
                assets.push(UnifiedAsset {
                    user: user_name.to_string(),
                    agent: agent_name.clone(),
                    agent_title: agent_title.clone(),
                    agent_home: agent_home.clone(),
                    asset_type: *asset_type,
                    data,
                });
            }
        }
    }
    Ok(assets)
}

/// Scan skills from agents or a directory.
pub fn scan_skills(request: &ScanRequest) -> SentraResult<Vec<SkillScanResult>> {
    let request = request.clone();
    block_on(async move { scan_skills_async(&request).await })
}

pub async fn scan_skills_async(request: &ScanRequest) -> SentraResult<Vec<SkillScanResult>> {
    let home = resolve_home(request.home.as_deref())?;
    let options = build_scan_options(&home, request.checkers.as_ref())?;
    let targets = if let Some(ref path) = request.path {
        collect_path_targets_async(path).await?
    } else {
        collect_agent_targets_async(&home, request.agents.as_deref().unwrap_or(&[])).await?
    };

    let mut results = Vec::with_capacity(targets.len());
    if targets.is_empty() {
        return Ok(results);
    }
    let scanner = RiskScanner::new(options)?;
    for target in targets {
        let report = scanner.scan(RiskAsset::from(&target.skill)).await?;
        results.push(SkillScanResult {
            asset_type: AssetType::Skill,
            source: target.source,
            agent_name: target.agent_name,
            agent_title: target.agent_title,
            agent_home: target.agent_home,
            data: target.skill,
            report,
        });
    }
    Ok(results)
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build Tokio runtime for C binding")
        .block_on(future)
}

fn build_scan_options(
    home: &Path,
    checkers: Option<&ScannerSelection>,
) -> SentraResult<ScanOptions> {
    let mut options = load_scan_config(home)?;
    apply_default_rule_dirs(home, &mut options);
    let checkers = checkers.cloned().unwrap_or_default();
    options.checker = Some(CheckerConfig {
        enable_hash: checkers.hash,
        enable_yara: checkers.yara,
        enable_local_ti: checkers.ti,
        enable_llm: checkers.llm,
        enable_online_ti: checkers.online_ti,
    });
    Ok(options)
}

fn load_scan_config(home: &Path) -> SentraResult<ScanOptions> {
    let path = runtime::active_config_file(home);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Ok(ScanOptions::default());
    };
    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|err| SentraError::Message(err.to_string()))?;
    let raw = value.get("scan").unwrap_or(&value);
    let mut options: ScanOptions =
        serde_json::from_value(raw.clone()).map_err(|err| SentraError::Message(err.to_string()))?;
    merge_legacy_sentra_config(&value, &mut options);
    Ok(options)
}

fn apply_default_rule_dirs(home: &Path, options: &mut ScanOptions) {
    let rules = options
        .rules
        .get_or_insert_with(RuleDirectoryConfig::default);
    if rules.hash.is_none() {
        rules.hash = Some(runtime::active_hash_rule_dir(home));
    }
    if rules.yara.is_none() {
        rules.yara = Some(runtime::active_yara_rule_dir(home));
    }
    if rules.ti.is_none() {
        rules.ti = Some(runtime::active_ti_rule_dir(home));
    }
}

fn merge_legacy_sentra_config(config: &serde_json::Value, options: &mut ScanOptions) {
    if let Some(llm) = config.get("llm").and_then(|value| value.as_object()) {
        let options_llm = options.llm.get_or_insert_with(LlmConfig::default);
        if options_llm.api_url.is_none() {
            options_llm.api_url = llm
                .get("apiUrl")
                .or_else(|| llm.get("api"))
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        if options_llm.api_key.is_none() {
            options_llm.api_key = llm
                .get("apiKey")
                .or_else(|| llm.get("key"))
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        if options_llm.model.is_none() {
            options_llm.model = llm
                .get("model")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        if options_llm.protocol.is_none() {
            options_llm.protocol = llm
                .get("protocol")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok());
        }
        if options_llm.max_tokens.is_none() {
            options_llm.max_tokens = llm
                .get("maxTokens")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize);
        }
        if options_llm.max_prompt_chars.is_none() {
            options_llm.max_prompt_chars = llm
                .get("maxPromptChars")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize);
        }
        if options_llm.timeout_ms.is_none() {
            options_llm.timeout_ms = llm.get("timeoutMs").and_then(|value| value.as_u64());
        }
        if options_llm.stream.is_none() {
            options_llm.stream = llm.get("stream").and_then(|value| value.as_bool());
        }
        if options_llm.prompt.is_none() {
            options_llm.prompt = llm
                .get("prompt")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
    }
    if options.rules.is_none()
        && let Some(rules) = config.get("rules").cloned()
    {
        options.rules = serde_json::from_value::<RuleDirectoryConfig>(rules).ok();
    }
}

async fn collect_path_targets_async(path: &Path) -> SentraResult<Vec<SkillScanTarget>> {
    let path = canonical_path(path)?;
    let skills = crate::collect_skills_from_dir_async(&path).await?;
    Ok(skills
        .into_iter()
        .map(|skill| SkillScanTarget {
            source: "path".to_string(),
            agent_name: None,
            agent_title: None,
            agent_home: Some(path.clone()),
            skill,
        })
        .collect())
}

async fn collect_agent_targets_async(
    home: &Path,
    agent_filters: &[String],
) -> SentraResult<Vec<SkillScanTarget>> {
    let mut targets = Vec::new();
    for agent in discover_agents(home) {
        if !agent_filters.is_empty()
            && !agent_filters
                .iter()
                .any(|filter| agent_matches(filter, agent.name()))
        {
            continue;
        }
        let agent_name = agent.name().to_string();
        let agent_title = agent.title().to_string();
        let agent_home = agent.home().to_path_buf();
        for skill in collect_skill_assets_async(&agent).await? {
            targets.push(SkillScanTarget {
                source: "agent".to_string(),
                agent_name: Some(agent_name.clone()),
                agent_title: Some(agent_title.clone()),
                agent_home: Some(agent_home.clone()),
                skill,
            });
        }
    }
    Ok(targets)
}

async fn collect_skill_assets_async(agent: &crate::agents::Agent) -> SentraResult<Vec<SkillData>> {
    let mut skills = Vec::new();
    for asset in agent.get_assets(AssetType::Skill)? {
        let data = asset.data_async().await?;
        let mut asset_skills: Vec<SkillData> =
            serde_json::from_value(data).map_err(|err| SentraError::Message(err.to_string()))?;
        skills.append(&mut asset_skills);
    }
    Ok(skills)
}

fn canonical_path(path: &Path) -> SentraResult<PathBuf> {
    let path = path
        .canonicalize()
        .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))?;
    Ok(clean_canonical_path(path))
}

fn clean_canonical_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = value.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

fn agent_matches(filter: &str, agent_name: &str) -> bool {
    filter == agent_name || (filter == "claude" && agent_name.starts_with("claude-"))
}

fn resolve_home(home: Option<&Path>) -> SentraResult<PathBuf> {
    match home {
        Some(path) => Ok(path.to_path_buf()),
        None => home::home_dir()
            .ok_or_else(|| SentraError::Message("could not determine home".to_string())),
    }
}

fn is_empty_data(data: &serde_json::Value) -> bool {
    data.as_array().is_some_and(|items| items.is_empty())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillScanResult {
    pub asset_type: AssetType,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_home: Option<PathBuf>,
    pub data: SkillData,
    pub report: ScanReport,
}

struct SkillScanTarget {
    source: String,
    agent_name: Option<String>,
    agent_title: Option<String>,
    agent_home: Option<PathBuf>,
    skill: SkillData,
}

#[cfg(test)]
mod tests {
    use super::super::types::ScannerSelection;
    use super::*;
    use std::fs;

    #[test]
    fn collects_codex_skill_asset() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join(".codex").join("skills").join("demo");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: demo\ndescription: A demo skill\n---\nbody",
        )
        .unwrap();

        let assets = collect_all_assets(Some(dir.path())).unwrap();
        let skills: Vec<_> = assets
            .iter()
            .filter(|a| a.asset_type == AssetType::Skill)
            .collect();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].agent, "codex");
        assert_eq!(skills[0].agent_title, "Codex CLI");
        assert_eq!(skills[0].asset_type, AssetType::Skill);
        assert_eq!(skills[0].data[0]["name"], "demo");
        assert_eq!(skills[0].data[0]["description"], "A demo skill");
    }

    #[test]
    fn collects_assets_through_async_adapter() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join(".codex").join("skills").join("demo");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: demo\ndescription: A demo skill\n---\nbody",
        )
        .unwrap();

        let assets = block_on(collect_all_assets_async(Some(dir.path()))).unwrap();
        let skills: Vec<_> = assets
            .iter()
            .filter(|a| a.asset_type == AssetType::Skill)
            .collect();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].agent, "codex");
        assert_eq!(skills[0].data[0]["name"], "demo");
    }

    #[test]
    fn empty_skill_array_is_filtered_out() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".codex")).unwrap();

        let assets = collect_all_assets(Some(dir.path())).unwrap();
        // Only codex agent discovered; skills/mcp/provider/cron are empty
        // and should be filtered out. Memory may report .codex-global-state.json.
        let skills: Vec<_> = assets
            .iter()
            .filter(|a| a.asset_type == AssetType::Skill)
            .collect();
        assert!(
            skills.is_empty(),
            "no skills dir exists, skill should be empty"
        );
        let mcps: Vec<_> = assets
            .iter()
            .filter(|a| a.asset_type == AssetType::Mcp)
            .collect();
        assert!(mcps.is_empty(), "no config.toml, mcp should be empty");
    }

    #[test]
    fn missing_home_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let nonexistent = dir.path().join("nonexistent");
        let assets = collect_all_assets(Some(&nonexistent)).unwrap();
        assert!(assets.is_empty());
    }

    #[test]
    fn scan_skill_from_path_returns_report() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\ndescription: For testing\n---\nbody",
        )
        .unwrap();

        let request = ScanRequest {
            home: None,
            path: Some(skill_dir.clone()),
            agents: None,
            checkers: None,
        };
        let results = scan_skills(&request).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "path");
        assert_eq!(results[0].data.name, "test-skill");
        assert_eq!(results[0].asset_type, AssetType::Skill);
        assert!(results[0].agent_name.is_none());
        // Report should have metadata, summary, and empty findings
        assert_eq!(results[0].report.summary.critical, 0);
        assert_eq!(results[0].report.findings.len(), 0);
    }

    #[test]
    fn scan_skill_from_agent_filters_by_name() {
        let dir = tempfile::tempdir().unwrap();
        // Create codex with a skill
        let codex_skill = dir.path().join(".codex").join("skills").join("codex-skill");
        fs::create_dir_all(&codex_skill).unwrap();
        fs::write(
            codex_skill.join("SKILL.md"),
            "---\nname: codex-skill\n---\nbody",
        )
        .unwrap();

        let request = ScanRequest {
            home: Some(dir.path().to_path_buf()),
            path: None,
            agents: Some(vec!["codex".to_string()]),
            checkers: None,
        };
        let results = scan_skills(&request).unwrap();
        let codex_results: Vec<_> = results
            .iter()
            .filter(|r| r.agent_name.as_deref() == Some("codex"))
            .collect();
        assert_eq!(codex_results.len(), 1);
        assert_eq!(codex_results[0].data.name, "codex-skill");
    }

    #[test]
    fn scan_skill_merges_legacy_llm_config() {
        let dir = tempfile::tempdir().unwrap();
        let sentra_home = dir.path().join(".sentra");
        fs::create_dir_all(&sentra_home).unwrap();
        fs::write(
            sentra_home.join("config.json"),
            r#"{
  "llm": {
    "api": "offline://fixture",
    "key": "test-key",
    "model": "test-model",
    "prompt": "{\"results\":[{\"findings\":[{\"severity\":\"HIGH\",\"category\":\"PROMPT_INJECTION\",\"title\":\"LLM reviewed\",\"description\":\"confirmed\",\"evidence\":\"legacy-adapter-marker\",\"remediation\":\"remove\"}]}]}"
  }
}"#,
        )
        .unwrap();
        let skill_dir = dir.path().join("adapter-scan");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: adapter-scan\n---\nlegacy-adapter-marker",
        )
        .unwrap();

        let request = ScanRequest {
            home: Some(dir.path().to_path_buf()),
            path: Some(skill_dir),
            agents: None,
            checkers: Some(ScannerSelection {
                hash: Some(false),
                yara: Some(false),
                ti: Some(false),
                llm: Some(true),
                online_ti: Some(false),
            }),
        };
        let results = scan_skills(&request).unwrap();
        let report = &results[0].report;

        assert!(report.errors.is_empty());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.checker == "llm-checker")
        );
    }

    #[test]
    fn scan_skill_nonexistent_path_returns_error() {
        let request = ScanRequest {
            home: None,
            path: Some(PathBuf::from("/tmp/nonexistent-sentra-test-dir")),
            agents: None,
            checkers: None,
        };
        assert!(scan_skills(&request).is_err());
    }

    // ── collect_all_assets: multi asset-type coverage ─────────────────

    #[test]
    fn collect_all_asset_types() {
        let dir = tempfile::tempdir().unwrap();
        let codex_home = dir.path().join(".codex");
        fs::create_dir_all(&codex_home).unwrap();

        // Skill
        let skill_dir = codex_home.join("skills").join("multi-type-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: multi-type-skill\n---\nbody",
        )
        .unwrap();

        // MCP + Provider: both read from config.toml
        fs::write(
            codex_home.join("config.toml"),
            r#"[mcp_servers.test-mcp]
command = "echo"
args = ["hello"]

[model_providers.test-provider]
name = "Test Provider"
base_url = "https://api.example.com/v1"
"#,
        )
        .unwrap();

        // Cron: needs automations/<id>/automation.toml
        let cron_dir = codex_home.join("automations").join("daily-scan");
        fs::create_dir_all(&cron_dir).unwrap();
        fs::write(
            cron_dir.join("automation.toml"),
            r#"id = "daily-scan"
name = "Daily Security Scan"
prompt = "Run security scan"
rrule = "FREQ=DAILY"
status = "ACTIVE"
"#,
        )
        .unwrap();

        let plugin_dir = codex_home
            .join("plugins")
            .join("cache")
            .join("market")
            .join("stable")
            .join("demo-plugin")
            .join(".codex-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name":"demo-plugin","version":"1.0.0"}"#,
        )
        .unwrap();

        let assets = collect_all_assets(Some(dir.path())).unwrap();

        // Verify all 6 asset types appear
        let types: Vec<AssetType> = assets.iter().map(|a| a.asset_type).collect();
        assert!(
            types.contains(&AssetType::Skill),
            "should contain Skill asset"
        );
        assert!(types.contains(&AssetType::Mcp), "should contain Mcp asset");
        assert!(
            types.contains(&AssetType::Provider),
            "should contain Provider asset"
        );
        assert!(
            types.contains(&AssetType::Cron),
            "should contain Cron asset"
        );
        // Memory: Codex always reports .codex-global-state.json even when
        // the file doesn't exist — so Memory should appear.
        assert!(
            types.contains(&AssetType::Memory),
            "should contain Memory asset"
        );
        assert!(
            types.contains(&AssetType::Plugin),
            "should contain Plugin asset"
        );

        // Verify MCP data content
        let mcp_asset = assets
            .iter()
            .find(|a| a.asset_type == AssetType::Mcp)
            .unwrap();
        assert!(mcp_asset.data.is_array());
        let mcp_items = mcp_asset.data.as_array().unwrap();
        assert!(mcp_items.iter().any(|item| item["name"] == "test-mcp"));

        // Verify Provider data content
        let provider_asset = assets
            .iter()
            .find(|a| a.asset_type == AssetType::Provider)
            .unwrap();
        let provider_items = provider_asset.data.as_array().unwrap();
        assert!(
            provider_items
                .iter()
                .any(|item| item["name"] == "Test Provider")
        );

        // Verify Cron data content
        let cron_asset = assets
            .iter()
            .find(|a| a.asset_type == AssetType::Cron)
            .unwrap();
        let cron_items = cron_asset.data.as_array().unwrap();
        assert!(
            cron_items
                .iter()
                .any(|item| item["name"] == "Daily Security Scan")
        );

        let plugin_asset = assets
            .iter()
            .find(|a| a.asset_type == AssetType::Plugin)
            .unwrap();
        let plugin_items = plugin_asset.data.as_array().unwrap();
        assert!(
            plugin_items
                .iter()
                .any(|item| item["name"] == "demo-plugin")
        );
    }

    #[test]
    fn collect_user_field_matches_home_dir_name() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join(".codex").join("skills").join("user-test");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: user-test\n---\nbody",
        )
        .unwrap();

        let assets = collect_all_assets(Some(dir.path())).unwrap();
        let expected_user = dir.path().file_name().unwrap().to_str().unwrap();

        for asset in &assets {
            assert_eq!(
                asset.user, expected_user,
                "user field must match home dir name"
            );
        }
        assert!(!assets.is_empty(), "should have at least one asset");
    }

    // ── scan_skills: agent filter edge cases ───────────────────────────

    #[test]
    fn scan_agent_filter_no_match_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        // Create codex with a skill so there's something to discover
        let skill_dir = dir.path().join(".codex").join("skills").join("no-match");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: no-match\n---\nbody").unwrap();

        let request = ScanRequest {
            home: Some(dir.path().to_path_buf()),
            path: None,
            agents: Some(vec!["nonexistent-agent".to_string()]),
            checkers: None,
        };
        let results = scan_skills(&request).unwrap();
        assert!(
            results.is_empty(),
            "non-matching agent filter must return empty results"
        );
    }

    #[test]
    fn scan_agent_filter_claude_matches_claude_cli() {
        let dir = tempfile::tempdir().unwrap();
        // Create .claude/ dir to trigger Claude CLI agent discovery
        // (CLAUDE_CLI_AGENT_ENTRY has name "claude-cli", homes: [".claude"])
        let claude_home = dir.path().join(".claude");
        let skill_dir = claude_home.join("skills").join("claude-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: claude-skill\n---\nbody",
        )
        .unwrap();

        // Filter "codex" should NOT match claude-cli → empty
        let request_codex = ScanRequest {
            home: Some(dir.path().to_path_buf()),
            path: None,
            agents: Some(vec!["codex".to_string()]),
            checkers: None,
        };
        let results = scan_skills(&request_codex).unwrap();
        let claude_results: Vec<_> = results
            .iter()
            .filter(|r| r.agent_name.as_deref() == Some("claude-cli"))
            .collect();
        assert!(
            claude_results.is_empty(),
            "filter 'codex' must not match claude-cli"
        );

        // Filter "claude" SHOULD match claude-cli via agent_matches wildcard
        let request_claude = ScanRequest {
            home: Some(dir.path().to_path_buf()),
            path: None,
            agents: Some(vec!["claude".to_string()]),
            checkers: None,
        };
        let results = scan_skills(&request_claude).unwrap();
        let matched: Vec<_> = results
            .iter()
            .filter(|r| r.agent_name.as_deref() == Some("claude-cli"))
            .collect();
        assert_eq!(
            matched.len(),
            1,
            "filter 'claude' must match claude-cli via wildcard"
        );
        assert_eq!(matched[0].data.name, "claude-skill");
    }

    #[test]
    fn scan_path_wins_over_agents() {
        let dir = tempfile::tempdir().unwrap();

        // Create a skill in a standalone dir (not inside any agent home)
        let skill_dir = dir.path().join("standalone-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: standalone\n---\nbody",
        )
        .unwrap();

        // Also create a codex skill — if path didn't win, scan_skills
        // would scan codex agent skills instead.
        let codex_skill = dir.path().join(".codex").join("skills").join("codex-other");
        fs::create_dir_all(&codex_skill).unwrap();
        fs::write(
            codex_skill.join("SKILL.md"),
            "---\nname: codex-other\n---\nbody",
        )
        .unwrap();

        // Set both path and agents — path should win
        let request = ScanRequest {
            home: Some(dir.path().to_path_buf()),
            path: Some(skill_dir.clone()),
            agents: Some(vec!["codex".to_string()]),
            checkers: None,
        };
        let results = scan_skills(&request).unwrap();
        assert_eq!(results.len(), 1, "path mode: only the standalone skill");
        assert_eq!(results[0].source, "path");
        assert_eq!(results[0].data.name, "standalone");
        assert!(
            results[0].agent_name.is_none(),
            "path scan has no agent context"
        );
    }
}
