use std::path::Path;

use crate::interfaces::{AssetType, ErasedAsset, ProcessData};

pub(crate) type AgentAssetFactory = fn(&str, &Path, AssetType) -> Vec<Box<dyn ErasedAsset>>;
pub(crate) type AgentInstallDetector = fn(&str, &Path) -> bool;
pub(crate) type AgentProcessProvider = fn() -> Vec<ProcessData>;

#[derive(Debug, Clone)]
pub(crate) struct AgentEntry {
    pub(crate) name: &'static str,
    pub(crate) title: Option<&'static str>,
    pub(crate) homes: &'static [&'static [&'static str]],
    pub(crate) asset_for_type: AgentAssetFactory,
    pub(crate) is_installed: AgentInstallDetector,
    pub(crate) process_provider: AgentProcessProvider,
    pub(crate) process_home_env_vars: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SystemAgentPath {
    pub(crate) entry: &'static AgentEntry,
    pub(crate) system_path: &'static str,
}

pub(crate) const CODEX_CLI_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "codex-cli",
    title: Some("Codex CLI"),
    homes: &[&[".codex"]],
    asset_for_type: crate::agents::codex::asset_for_type,
    is_installed: crate::agents::codex::is_agent_installed,
    process_provider: crate::agents::codex::process_data,
    process_home_env_vars: &["CODEX_HOME"],
};

pub(crate) const CODEX_APP_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "codex-app",
    title: Some("Codex App"),
    homes: &[&[".codex"]],
    asset_for_type: crate::agents::codex::asset_for_type,
    is_installed: crate::agents::codex::is_agent_installed,
    process_provider: crate::agents::codex::app_process_data,
    process_home_env_vars: &[],
};

pub(crate) const CODEX_CLI_IDE_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "codex-cli-ide",
    title: Some("Codex IDE Extension"),
    homes: &[&[".codex"]],
    asset_for_type: crate::agents::codex::asset_for_type,
    is_installed: crate::agents::codex::is_agent_installed,
    process_provider: crate::agents::codex::ide_process_data,
    process_home_env_vars: &[],
};

pub(crate) const CLAUDE_CLI_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "claude-cli",
    title: Some("Claude Code"),
    homes: &[&[".claude"]],
    asset_for_type: crate::agents::claude::asset_for_type,
    is_installed: crate::agents::claude::is_agent_installed,
    process_provider: crate::agents::claude::process_data,
    process_home_env_vars: &[],
};

pub(crate) const CLAUDE_CLI_IDE_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "claude-cli-ide",
    title: Some("Claude Code IDE Extension"),
    homes: &[&[".claude"]],
    asset_for_type: crate::agents::claude::asset_for_type,
    is_installed: crate::agents::claude::is_agent_installed,
    process_provider: crate::agents::claude::ide_process_data,
    process_home_env_vars: &[],
};

pub(crate) const CLAUDE_APP_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "claude-app",
    title: Some("Claude App"),
    homes: &[
        &["AppData", "Local", "Claude"],
        &["AppData", "Local", "Claude-3p"],
        &["Library", "Application Support", "Claude"],
        &["Library", "Application Support", "Claude-3p"],
    ],
    asset_for_type: crate::agents::claude::asset_for_type,
    is_installed: crate::agents::claude::is_agent_installed,
    process_provider: crate::agents::claude::app_process_data,
    process_home_env_vars: &[],
};

pub(crate) const HERMES_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "hermes",
    title: Some("Hermes"),
    homes: &[&[".hermes"]],
    asset_for_type: crate::agents::hermes::asset_for_type,
    is_installed: crate::agents::hermes::is_agent_installed,
    process_provider: crate::agents::hermes::process_data,
    process_home_env_vars: &[],
};

pub(crate) const KIMI_CLI_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "kimi-cli",
    title: Some("Kimi Code"),
    homes: &[&[".kimi-code"]],
    asset_for_type: crate::agents::kimi::asset_for_type,
    is_installed: crate::agents::kimi::is_agent_installed,
    process_provider: crate::agents::kimi::process_data,
    process_home_env_vars: &["KIMI_CODE_HOME"],
};

pub(crate) const KIMI_APP_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "kimi-app",
    title: Some("Kimi App"),
    homes: &[
        &["AppData", "Roaming", "kimi-desktop"],
        &["Library", "Application Support", "kimi-desktop"],
        &[".config", "kimi-desktop"],
    ],
    asset_for_type: crate::agents::kimi::asset_for_type,
    is_installed: crate::agents::kimi::is_agent_installed,
    process_provider: crate::agents::kimi::app_process_data,
    process_home_env_vars: &[],
};

pub(crate) const KIMI_CLI_IDE_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "kimi-cli-ide",
    title: Some("Kimi Code IDE Extension"),
    homes: &[&[".kimi-code"]],
    asset_for_type: crate::agents::kimi::asset_for_type,
    is_installed: crate::agents::kimi::is_agent_installed,
    process_provider: crate::agents::kimi::ide_process_data,
    process_home_env_vars: &["KIMI_CODE_HOME"],
};

pub(crate) const OPENCLAW_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "openclaw",
    title: Some("OpenClaw"),
    homes: &[&[".openclaw"]],
    asset_for_type: crate::agents::openclaw::asset_for_type,
    is_installed: crate::agents::openclaw::is_agent_installed,
    process_provider: crate::agents::openclaw::process_data,
    process_home_env_vars: &[],
};

pub(crate) const OPENCODE_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "opencode",
    title: Some("OpenCode"),
    homes: &[&[".config", "opencode"]],
    asset_for_type: crate::agents::opencode::asset_for_type,
    is_installed: crate::agents::opencode::is_agent_installed,
    process_provider: crate::agents::opencode::process_data,
    process_home_env_vars: &[],
};

pub(crate) const PI_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "pi",
    title: Some("Pi"),
    homes: &[&[".pi", "agent"]],
    asset_for_type: crate::agents::pi::asset_for_type,
    is_installed: crate::agents::pi::is_agent_installed,
    process_provider: crate::agents::pi::process_data,
    process_home_env_vars: &[],
};

pub(crate) const SENTRA_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "sentra",
    title: Some("Sentra"),
    homes: &[&[crate::config::SENTRA_HOME_DIR_NAME]],
    asset_for_type: crate::agents::sentra::asset_for_type,
    is_installed: crate::agents::sentra::is_agent_installed,
    process_provider: crate::agents::sentra::process_data,
    process_home_env_vars: &[],
};

pub(crate) const ANTIGRAVITY_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "antigravity",
    title: Some("Antigravity"),
    homes: &[&[".gemini", "antigravity-cli"]],
    asset_for_type: crate::agents::antigravity::asset_for_type,
    is_installed: crate::agents::antigravity::is_agent_installed,
    process_provider: crate::agents::antigravity::process_data,
    process_home_env_vars: &[],
};

pub(crate) const CODEBUDDY_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "codebuddy",
    title: Some("CodeBuddy"),
    homes: &[&[".codebuddy"]],
    asset_for_type: crate::agents::codebuddy::asset_for_type,
    is_installed: crate::agents::codebuddy::is_agent_installed,
    process_provider: crate::agents::codebuddy::process_data,
    process_home_env_vars: &["CODEBUDDY_CONFIG_DIR"],
};

pub(crate) const CODER_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "coder",
    title: Some("Coder"),
    homes: &[&[".config", "coderv2"]],
    asset_for_type: crate::agents::coder::asset_for_type,
    is_installed: crate::agents::coder::is_agent_installed,
    process_provider: crate::agents::coder::process_data,
    process_home_env_vars: &["CODER_CONFIG_DIR"],
};

pub(crate) const CURSOR_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "cursor",
    title: Some("Cursor"),
    homes: &[&[".cursor"]],
    asset_for_type: crate::agents::cursor::asset_for_type,
    is_installed: crate::agents::cursor::is_agent_installed,
    process_provider: crate::agents::cursor::process_data,
    process_home_env_vars: &["CURSOR_CONFIG_DIR"],
};

pub(crate) const KIRO_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "kiro",
    title: Some("Kiro"),
    homes: &[&[".kiro"]],
    asset_for_type: crate::agents::kiro::asset_for_type,
    is_installed: crate::agents::kiro::is_agent_installed,
    process_provider: crate::agents::kiro::process_data,
    process_home_env_vars: &["KIRO_HOME"],
};

pub(crate) const LINGCODE_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "lingcode",
    title: Some("LingCode"),
    homes: &[&[".lingcode"]],
    asset_for_type: crate::agents::lingcode::asset_for_type,
    is_installed: crate::agents::lingcode::is_agent_installed,
    process_provider: crate::agents::lingcode::process_data,
    process_home_env_vars: &[],
};

pub(crate) const MARVIS_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "marvis",
    title: Some("MarvisX"),
    homes: &[&[".marvis"]],
    asset_for_type: crate::agents::marvis::asset_for_type,
    is_installed: crate::agents::marvis::is_agent_installed,
    process_provider: crate::agents::marvis::process_data,
    process_home_env_vars: &[],
};

pub(crate) const QODER_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "qoder",
    title: Some("Qoder"),
    homes: &[&[".qoder"]],
    asset_for_type: crate::agents::qoder::asset_for_type,
    is_installed: crate::agents::qoder::is_agent_installed,
    process_provider: crate::agents::qoder::process_data,
    process_home_env_vars: &[],
};

pub(crate) const QODER_CN_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "qoder-cn",
    title: Some("Qoder CN"),
    homes: &[&[".qoder-cn"]],
    asset_for_type: crate::agents::qoder::asset_for_type,
    is_installed: crate::agents::qoder::is_agent_installed,
    process_provider: crate::agents::qoder::process_data,
    process_home_env_vars: &[],
};

pub(crate) const QODER_AGENT_ENTRIES: &[AgentEntry] = &[QODER_AGENT_ENTRY, QODER_CN_AGENT_ENTRY];

pub(crate) const QODERWORK_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "qoderwork",
    title: Some("QoderWork"),
    homes: &[&[".qoderwork"]],
    asset_for_type: crate::agents::qoderwork::asset_for_type,
    is_installed: crate::agents::qoderwork::is_agent_installed,
    process_provider: crate::agents::qoderwork::process_data,
    process_home_env_vars: &[],
};

pub(crate) const TRAE_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "trae",
    title: Some("Trae"),
    homes: &[&[".trae"]],
    asset_for_type: crate::agents::trae::asset_for_type,
    is_installed: crate::agents::trae::is_agent_installed,
    process_provider: crate::agents::trae::process_data,
    process_home_env_vars: &[],
};

pub(crate) const VSCODE_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "vscode",
    title: Some("VS Code"),
    homes: &[&[".vscode"]],
    asset_for_type: crate::agents::vscode::asset_for_type,
    is_installed: crate::agents::vscode::is_agent_installed,
    process_provider: crate::agents::vscode::process_data,
    process_home_env_vars: &[],
};

pub(crate) const WORKBUDDY_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "workbuddy",
    title: Some("WorkBuddy"),
    homes: &[&[".workbuddy"]],
    asset_for_type: crate::agents::workbuddy::asset_for_type,
    is_installed: crate::agents::workbuddy::is_agent_installed,
    process_provider: crate::agents::workbuddy::process_data,
    process_home_env_vars: &[],
};

pub(crate) const HERMES_SYSTEM_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "hermes",
    title: Some("Hermes"),
    homes: &[],
    asset_for_type: crate::agents::hermes::asset_for_type,
    is_installed: crate::agents::hermes::is_agent_installed,
    process_provider: crate::agents::hermes::process_data,
    process_home_env_vars: &[],
};

pub(crate) const SYSTEM_AGENT_PATHS: &[SystemAgentPath] = &[
    #[cfg(unix)]
    SystemAgentPath {
        entry: &HERMES_SYSTEM_AGENT_ENTRY,
        system_path: "/usr/local/lib/hermes-agent",
    },
    #[cfg(unix)]
    SystemAgentPath {
        entry: &HERMES_SYSTEM_AGENT_ENTRY,
        system_path: "/opt/hermes-agent",
    },
];

pub(crate) const GENERAL_AGENT_ENTRIES: &[AgentEntry] = &[
    general("agents", &[&[".agents"]]),
    general("aider-desk", &[&[".aider-desk"]]),
    general("augment", &[&[".augment"]]),
    general("bob", &[&[".bob"]]),
    general("codearts-agent", &[&[".codeartsdoer"]]),
    general("codemaker", &[&[".codemaker"]]),
    general("codestudio", &[&[".codestudio"]]),
    general("command-code", &[&[".commandcode"]]),
    general("cortex", &[&[".snowflake", "cortex"]]),
    general("crush", &[&[".config", "crush"]]),
    general("deepagents", &[&[".deepagents", "agent"]]),
    general("devin", &[&[".devin"]]),
    general("droid", &[&[".factory"]]),
    general("firebender", &[&[".firebender"]]),
    general("forgecode", &[&[".forge"]]),
    general("gemini-cli", &[&[".gemini"]]),
    general("copilot", &[&[".copilot"]]),
    general("goose", &[&[".goose"]]),
    general("iflow-cli", &[&[".iflow"]]),
    general("jazz", &[&[".jazz"]]),
    general("junie", &[&[".junie"]]),
    general("kilo", &[&[".kilocode"]]),
    general("kode", &[&[".kode"]]),
    general("mcpjam", &[&[".mcpjam"]]),
    general("mistral-vibe", &[&[".vibe"]]),
    general("moxby", &[&[".moxby"]]),
    general("mux", &[&[".mux"]]),
    general("neovate", &[&[".neovate"]]),
    general("ona", &[&[".ona"]]),
    general("openhands", &[&[".openhands"]]),
    general("pochi", &[&[".pochi"]]),
    general("qwen-code", &[&[".qwen"]]),
    general("reasonix", &[&[".reasonix"]]),
    general("rovodev", &[&[".rovodev"]]),
    general("roo", &[&[".roo"]]),
    general("tabnine-cli", &[&[".tabnine", "agent"]]),
    general("terramind", &[&[".terramind"]]),
    general("tinycloud", &[&[".tinycloud"]]),
    general("trae-cn", &[&[".trae-cn"]]),
    general("windsurf", &[&[".codeium", "windsurf"]]),
    general("zencoder", &[&[".zencoder"]]),
    general("zenflow", &[&[".zencoder"]]),
];

pub(crate) fn builtin_agent_entries() -> Vec<AgentEntry> {
    let mut entries = vec![
        SENTRA_AGENT_ENTRY.clone(),
        CODEX_CLI_AGENT_ENTRY.clone(),
        CODEX_APP_AGENT_ENTRY.clone(),
        CODEX_CLI_IDE_AGENT_ENTRY.clone(),
        CLAUDE_CLI_AGENT_ENTRY.clone(),
        CLAUDE_CLI_IDE_AGENT_ENTRY.clone(),
        CLAUDE_APP_AGENT_ENTRY.clone(),
        HERMES_AGENT_ENTRY.clone(),
        KIMI_CLI_AGENT_ENTRY.clone(),
        KIMI_APP_AGENT_ENTRY.clone(),
        KIMI_CLI_IDE_AGENT_ENTRY.clone(),
        OPENCLAW_AGENT_ENTRY.clone(),
        OPENCODE_AGENT_ENTRY.clone(),
        PI_AGENT_ENTRY.clone(),
        HERMES_SYSTEM_AGENT_ENTRY.clone(),
        ANTIGRAVITY_AGENT_ENTRY.clone(),
        CODEBUDDY_AGENT_ENTRY.clone(),
        CODER_AGENT_ENTRY.clone(),
        CURSOR_AGENT_ENTRY.clone(),
        KIRO_AGENT_ENTRY.clone(),
        LINGCODE_AGENT_ENTRY.clone(),
        MARVIS_AGENT_ENTRY.clone(),
        QODER_AGENT_ENTRY.clone(),
        QODER_CN_AGENT_ENTRY.clone(),
        QODERWORK_AGENT_ENTRY.clone(),
        TRAE_AGENT_ENTRY.clone(),
        VSCODE_AGENT_ENTRY.clone(),
        WORKBUDDY_AGENT_ENTRY.clone(),
    ];
    entries.extend_from_slice(GENERAL_AGENT_ENTRIES);
    entries
}

const fn general(name: &'static str, homes: &'static [&'static [&'static str]]) -> AgentEntry {
    AgentEntry {
        name,
        title: None,
        homes,
        asset_for_type: crate::agents::general::asset_for_type,
        is_installed: crate::agents::general::is_agent_installed,
        process_provider: empty_process_data,
        process_home_env_vars: &[],
    }
}

pub(crate) fn empty_process_data() -> Vec<ProcessData> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn agent_entry_names_use_lowercase_kebab_case() {
        for entry in builtin_agent_entries() {
            assert!(
                entry.name.split('-').all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                }),
                "{}",
                entry.name
            );
        }
    }

    #[test]
    fn multi_surface_entries_use_canonical_names_and_titles() {
        for (entry, name, title) in [
            (&CODEX_CLI_AGENT_ENTRY, "codex-cli", "Codex CLI"),
            (&CODEX_APP_AGENT_ENTRY, "codex-app", "Codex App"),
            (
                &CODEX_CLI_IDE_AGENT_ENTRY,
                "codex-cli-ide",
                "Codex IDE Extension",
            ),
            (&CLAUDE_CLI_AGENT_ENTRY, "claude-cli", "Claude Code"),
            (&CLAUDE_APP_AGENT_ENTRY, "claude-app", "Claude App"),
            (
                &CLAUDE_CLI_IDE_AGENT_ENTRY,
                "claude-cli-ide",
                "Claude Code IDE Extension",
            ),
            (&KIMI_CLI_AGENT_ENTRY, "kimi-cli", "Kimi Code"),
            (&KIMI_APP_AGENT_ENTRY, "kimi-app", "Kimi App"),
            (
                &KIMI_CLI_IDE_AGENT_ENTRY,
                "kimi-cli-ide",
                "Kimi Code IDE Extension",
            ),
        ] {
            assert_eq!(entry.name, name);
            assert_eq!(entry.title, Some(title));
        }
    }

    #[test]
    fn concrete_agent_entries_route_process_assets() {
        for entry in [
            &CODEX_CLI_AGENT_ENTRY,
            &CODEX_APP_AGENT_ENTRY,
            &CODEX_CLI_IDE_AGENT_ENTRY,
            &CLAUDE_CLI_AGENT_ENTRY,
            &CLAUDE_CLI_IDE_AGENT_ENTRY,
            &CLAUDE_APP_AGENT_ENTRY,
            &HERMES_AGENT_ENTRY,
            &KIMI_CLI_AGENT_ENTRY,
            &KIMI_APP_AGENT_ENTRY,
            &KIMI_CLI_IDE_AGENT_ENTRY,
            &OPENCLAW_AGENT_ENTRY,
            &OPENCODE_AGENT_ENTRY,
            &PI_AGENT_ENTRY,
            &SENTRA_AGENT_ENTRY,
            &ANTIGRAVITY_AGENT_ENTRY,
            &CODEBUDDY_AGENT_ENTRY,
            &CODER_AGENT_ENTRY,
            &CURSOR_AGENT_ENTRY,
            &KIRO_AGENT_ENTRY,
            &LINGCODE_AGENT_ENTRY,
            &MARVIS_AGENT_ENTRY,
            &QODER_AGENT_ENTRY,
            &QODER_CN_AGENT_ENTRY,
            &QODERWORK_AGENT_ENTRY,
            &TRAE_AGENT_ENTRY,
            &VSCODE_AGENT_ENTRY,
            &WORKBUDDY_AGENT_ENTRY,
        ] {
            let assets =
                (entry.asset_for_type)(entry.name, Path::new("agent-home"), AssetType::Process);

            assert_eq!(assets.len(), 1, "{}", entry.name);
            assert_eq!(assets[0].asset_type(), AssetType::Process);
        }

        let general = &GENERAL_AGENT_ENTRIES[0];
        let assets =
            (general.asset_for_type)(general.name, Path::new("agent-home"), AssetType::Process);
        assert!(assets.is_empty());
    }
}
