use std::path::Path;

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
        std::slice::from_ref(&crate::agents::entries::ANTIGRAVITY_AGENT_ENTRY),
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
    agent_home
        .parent()
        .and_then(Path::parent)
        .unwrap_or(agent_home)
}

pub(super) fn plugin_manifests(agent_home: &Path) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    for root in [
        agent_home.join("plugins"),
        user_home(agent_home).join("plugins"),
    ] {
        paths.extend(find_named_files(&root, "plugin.json", 4, 0));
    }
    paths
}

pub(super) fn find_named_files(
    dir: &Path,
    file_name: &str,
    max_depth: usize,
    depth: usize,
) -> Vec<std::path::PathBuf> {
    if depth > max_depth || !crate::utils::dir_exists(dir) {
        return Vec::new();
    }
    let candidate = dir.join(file_name);
    if candidate.is_file() {
        return vec![candidate];
    }
    let mut results = Vec::new();
    for entry in std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if crate::utils::is_directory(&path) {
            results.extend(find_named_files(&path, file_name, max_depth, depth + 1));
        }
    }
    results
}
