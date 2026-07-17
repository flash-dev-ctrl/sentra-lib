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

pub(crate) const CODEX_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "codex",
    title: Some("Codex"),
    homes: &[&[".codex"]],
    asset_for_type: crate::agents::codex::asset_for_type,
    is_installed: crate::agents::codex::is_agent_installed,
    process_provider: crate::agents::codex::process_data,
    process_home_env_vars: &["CODEX_HOME"],
};

pub(crate) const CLAUDE_CLI_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "claude-cli",
    title: Some("Claude Code"),
    homes: &[&[".claude"]],
    asset_for_type: crate::agents::claude_cli::asset_for_type,
    is_installed: crate::agents::claude_cli::is_agent_installed,
    process_provider: crate::agents::claude_cli::process_data,
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
    asset_for_type: crate::agents::claude_app::asset_for_type,
    is_installed: crate::agents::claude_app::is_agent_installed,
    process_provider: crate::agents::claude_app::process_data,
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
    SystemAgentPath {
        entry: &HERMES_SYSTEM_AGENT_ENTRY,
        system_path: "/usr/local/lib/hermes-agent",
    },
    SystemAgentPath {
        entry: &HERMES_SYSTEM_AGENT_ENTRY,
        system_path: "/opt/hermes-agent",
    },
];

pub(crate) const GENERAL_AGENT_ENTRIES: &[AgentEntry] = &[
    general("aider-desk", &[&[".aider-desk"]]),
    general("augment", &[&[".augment"]]),
    general("bob", &[&[".bob"]]),
    general("codearts-agent", &[&[".codeartsdoer"]]),
    general("codebuddy", &[&[".codebuddy"]]),
    general("codemaker", &[&[".codemaker"]]),
    general("codestudio", &[&[".codestudio"]]),
    general("command-code", &[&[".commandcode"]]),
    general("continue", &[&[".continue"]]),
    general("cortex", &[&[".snowflake", "cortex"]]),
    general("crush", &[&[".config", "crush"]]),
    general("cursor", &[&[".cursor"]]),
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
    general("kiro-cli", &[&[".kiro"]]),
    general("kode", &[&[".kode"]]),
    general("lingma", &[&[".lingma"]]),
    general("mcpjam", &[&[".mcpjam"]]),
    general("mistral-vibe", &[&[".vibe"]]),
    general("moxby", &[&[".moxby"]]),
    general("mux", &[&[".mux"]]),
    general("neovate", &[&[".neovate"]]),
    general("ona", &[&[".ona"]]),
    general("openhands", &[&[".openhands"]]),
    general("pochi", &[&[".pochi"]]),
    general("qoder", &[&[".qoder"]]),
    general("qoder-cn", &[&[".qoder-cn"]]),
    general("qwen-code", &[&[".qwen"]]),
    general("reasonix", &[&[".reasonix"]]),
    general("rovodev", &[&[".rovodev"]]),
    general("roo", &[&[".roo"]]),
    general("tabnine-cli", &[&[".tabnine", "agent"]]),
    general("terramind", &[&[".terramind"]]),
    general("tinycloud", &[&[".tinycloud"]]),
    general("trae", &[&[".trae"]]),
    general("trae-cn", &[&[".trae-cn"]]),
    general("windsurf", &[&[".codeium", "windsurf"]]),
    general("zencoder", &[&[".zencoder"]]),
    general("zenflow", &[&[".zencoder"]]),
];

pub(crate) fn builtin_agent_entries() -> Vec<AgentEntry> {
    let mut entries = vec![
        SENTRA_AGENT_ENTRY.clone(),
        CODEX_AGENT_ENTRY.clone(),
        CLAUDE_CLI_AGENT_ENTRY.clone(),
        CLAUDE_APP_AGENT_ENTRY.clone(),
        HERMES_AGENT_ENTRY.clone(),
        OPENCLAW_AGENT_ENTRY.clone(),
        OPENCODE_AGENT_ENTRY.clone(),
        PI_AGENT_ENTRY.clone(),
        HERMES_SYSTEM_AGENT_ENTRY.clone(),
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
    fn concrete_agent_entries_route_process_assets() {
        for entry in [
            &CODEX_AGENT_ENTRY,
            &CLAUDE_CLI_AGENT_ENTRY,
            &CLAUDE_APP_AGENT_ENTRY,
            &HERMES_AGENT_ENTRY,
            &OPENCLAW_AGENT_ENTRY,
            &OPENCODE_AGENT_ENTRY,
            &PI_AGENT_ENTRY,
            &SENTRA_AGENT_ENTRY,
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
