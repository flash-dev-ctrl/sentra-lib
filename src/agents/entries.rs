use std::path::Path;

use crate::interfaces::{AssetType, ErasedAsset};

pub(crate) type AgentAssetFactory = fn(&str, &Path, AssetType) -> Vec<Box<dyn ErasedAsset>>;

#[derive(Debug, Clone)]
pub(crate) struct AgentEntry {
    pub(crate) name: &'static str,
    pub(crate) title: Option<&'static str>,
    pub(crate) homes: &'static [&'static [&'static str]],
    pub(crate) asset_for_type: AgentAssetFactory,
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
};

pub(crate) const CLAUDE_CLI_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "claude-cli",
    title: Some("Claude Code"),
    homes: &[&[".claude"]],
    asset_for_type: crate::agents::claude_cli::asset_for_type,
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
};

pub(crate) const HERMES_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "hermes",
    title: Some("Hermes"),
    homes: &[&[".hermes"]],
    asset_for_type: crate::agents::hermes::asset_for_type,
};

pub(crate) const OPENCLAW_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "openclaw",
    title: Some("OpenClaw"),
    homes: &[&[".openclaw"]],
    asset_for_type: crate::agents::openclaw::asset_for_type,
};

pub(crate) const SENTRA_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "sentra",
    title: Some("Sentra"),
    homes: &[&[crate::config::SENTRA_HOME_DIR_NAME]],
    asset_for_type: crate::agents::sentra::asset_for_type,
};

pub(crate) const HERMES_SYSTEM_AGENT_ENTRY: AgentEntry = AgentEntry {
    name: "hermes",
    title: Some("Hermes"),
    homes: &[],
    asset_for_type: crate::agents::hermes::asset_for_type,
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
    general("github-copilot", &[&[".copilot"]]),
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
    general("pi", &[&[".pi", "agent"]]),
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
    }
}
