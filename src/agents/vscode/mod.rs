use std::path::{Path, PathBuf};

use crate::interfaces::{AssetType, ErasedAsset};

mod cron;
mod mcp;
mod meta;
mod plugin;
mod process;
mod provider;
mod skill;

pub(crate) fn discover_agents(user_home: impl AsRef<Path>) -> Vec<crate::agents::Agent> {
    crate::agents::discovery::discover_entry_agents(
        user_home.as_ref(),
        std::slice::from_ref(&crate::agents::entries::VSCODE_AGENT_ENTRY),
    )
}

pub(crate) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    meta::is_agent_installed(agent_name, agent_home)
}

pub(crate) fn process_data() -> Vec<crate::interfaces::ProcessData> {
    process::process_data()
}

pub(crate) fn asset_for_type(
    agent_name: &str,
    agent_home: &Path,
    asset_type: AssetType,
) -> Vec<Box<dyn ErasedAsset>> {
    match asset_type {
        AssetType::Meta => vec![Box::new(meta::MetaAsset::new(agent_name, agent_home))],
        AssetType::Skill => vec![Box::new(skill::SkillAsset::new(agent_name, agent_home))],
        AssetType::Mcp => vec![Box::new(mcp::McpAsset::new(agent_name, agent_home))],
        AssetType::Memory => Vec::new(),
        AssetType::Cron => vec![Box::new(cron::CronAsset::new(agent_name, agent_home))],
        AssetType::Provider => vec![Box::new(provider::ProviderAsset::new(
            agent_name, agent_home,
        ))],
        AssetType::Plugin => vec![Box::new(plugin::PluginAsset::new(agent_name, agent_home))],
        AssetType::Process => vec![Box::new(crate::agents::process::ProcessAsset::new(
            agent_name,
            agent_home,
            process::matches_process,
        ))],
    }
}

pub(super) fn user_home(agent_home: &Path) -> &Path {
    agent_home.parent().unwrap_or(agent_home)
}

pub(super) fn extension_dirs(agent_home: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for root in [
        agent_home.join("extensions"),
        user_home(agent_home).join(".vscode").join("extensions"),
    ] {
        dirs.extend(read_dir_paths(&root));
    }
    if let Some(root) = crate::agents::install_status::env_path("VSCODE_EXTENSIONS") {
        dirs.extend(read_dir_paths(&root));
    }
    dirs
}

pub(super) fn agent_plugin_manifests(agent_home: &Path) -> Vec<PathBuf> {
    let root = user_home(agent_home)
        .join(".vscode")
        .join("agentPlugins")
        .join("cache");
    let mut paths = find_named_files(&root, "plugin.json", 5, 0);
    paths.extend(find_named_files(&root, "package.json", 5, 0));
    paths
}

pub(super) fn read_dir_paths(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
}

pub(super) fn find_named_files(
    dir: &Path,
    file_name: &str,
    max_depth: usize,
    depth: usize,
) -> Vec<PathBuf> {
    if depth > max_depth || !crate::utils::dir_exists(dir) {
        return Vec::new();
    }
    let candidate = dir.join(file_name);
    if candidate.is_file() {
        return vec![candidate];
    }
    let mut results = Vec::new();
    for entry in read_dir_paths(dir) {
        if crate::utils::is_directory(&entry) {
            results.extend(find_named_files(&entry, file_name, max_depth, depth + 1));
        }
    }
    results
}

pub(super) fn is_agent_manifest(manifest: &serde_json::Value) -> bool {
    if manifest
        .get("contributes")
        .and_then(|value| value.as_object())
        .is_some_and(has_agent_contribution)
    {
        return true;
    }
    [
        "agents",
        "agent",
        "skills",
        "chatSkills",
        "agentPlugins",
        "mcpServers",
    ]
    .iter()
    .any(|key| manifest.get(*key).is_some_and(|value| !value.is_null()))
}

fn has_agent_contribution(contributes: &serde_json::Map<String, serde_json::Value>) -> bool {
    [
        "chatParticipants",
        "chatParticipant",
        "chatAgents",
        "chatSkills",
        "skills",
        "agentPlugins",
        "agentTools",
        "languageModelTools",
        "mcpServerDefinitionProviders",
    ]
    .iter()
    .any(|key| contributes.get(*key).is_some_and(|value| !value.is_null()))
}

#[cfg(test)]
mod tests {
    use super::is_agent_manifest;

    #[test]
    fn manifest_filter_requires_agent_contribution_point() {
        assert!(is_agent_manifest(&serde_json::json!({
            "contributes": { "chatParticipants": [] }
        })));
        assert!(!is_agent_manifest(&serde_json::json!({
            "contributes": { "themes": [] }
        })));
    }
}
