use std::path::{Path, PathBuf};

use crate::interfaces::{AssetType, ErasedAsset};

mod install;
mod mcp;
mod meta;
mod plugin;
mod process;
mod provider;
mod skill;

pub(crate) use install::{install_plans_for_platform, uninstall_plans_for_platform};

pub(crate) fn discover_agents(user_home: impl AsRef<Path>) -> Vec<crate::agents::Agent> {
    crate::agents::discovery::discover_entry_agents(
        user_home.as_ref(),
        std::slice::from_ref(&crate::agents::entries::CODER_AGENT_ENTRY),
    )
}

pub(crate) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    meta::is_agent_installed(agent_name, agent_home)
}

pub(crate) fn is_install_target_installed(agent_home: &Path) -> bool {
    meta::is_install_target_installed(agent_home)
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
        AssetType::Memory | AssetType::Cron => Vec::new(),
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

pub(super) fn config_home(agent_home: &Path) -> PathBuf {
    std::env::var_os("CODER_CONFIG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| agent_home.to_path_buf())
}

pub(super) fn user_home(agent_home: &Path) -> &Path {
    agent_home
        .parent()
        .and_then(Path::parent)
        .unwrap_or(agent_home)
}

pub(super) fn read_dir_paths(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
}
