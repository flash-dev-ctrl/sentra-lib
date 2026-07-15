use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::interfaces::{AssetType, ErasedAsset};

mod install;
mod mcp;
mod meta;
mod plugin;
mod provider;
mod skill;

pub(crate) use install::{install_plans_for_platform, uninstall_plan_for_platform};

pub(crate) fn discover_agents(user_home: impl AsRef<Path>) -> Vec<crate::agents::Agent> {
    crate::agents::discovery::discover_entry_agents(
        user_home.as_ref(),
        std::slice::from_ref(&crate::agents::entries::OPENCODE_AGENT_ENTRY),
    )
}

pub(crate) fn data_home(agent_home: &Path) -> PathBuf {
    user_home(agent_home)
        .join(".local")
        .join("share")
        .join("opencode")
}

pub(crate) fn config_homes(agent_home: &Path) -> Vec<PathBuf> {
    vec![agent_home.to_path_buf()]
}

pub(crate) fn config_files(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = user_home(agent_home);
    let mut files = vec![
        user_home.join(".opencode").join("opencode.json"),
        user_home
            .join(".config")
            .join("opencode")
            .join("opencode.json"),
    ];
    let primary = agent_home.join("opencode.json");
    if !files.iter().any(|file| file == &primary) {
        files.insert(0, primary);
    }
    files.dedup();
    files
}

fn user_home(agent_home: &Path) -> PathBuf {
    if agent_home.file_name() == Some(OsStr::new("opencode"))
        && agent_home.parent().and_then(Path::file_name) == Some(OsStr::new(".config"))
    {
        return agent_home
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| agent_home.to_path_buf());
    }
    if agent_home.file_name() == Some(OsStr::new(".opencode")) {
        return agent_home
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| agent_home.to_path_buf());
    }
    agent_home.to_path_buf()
}

pub(crate) fn asset_for_type(
    agent_name: &str,
    agent_home: &std::path::Path,
    asset_type: AssetType,
) -> Vec<Box<dyn ErasedAsset>> {
    match asset_type {
        AssetType::Meta => vec![Box::new(meta::MetaAsset::new(agent_name, agent_home))],
        AssetType::Skill => vec![Box::new(skill::SkillAsset::new(agent_name, agent_home))],
        AssetType::Mcp => vec![Box::new(mcp::McpAsset::new(agent_name, agent_home))],
        AssetType::Memory => Vec::new(),
        AssetType::Cron => Vec::new(),
        AssetType::Provider => vec![Box::new(provider::ProviderAsset::new(
            agent_name, agent_home,
        ))],
        AssetType::Plugin => vec![Box::new(plugin::PluginAsset::new(agent_name, agent_home))],
    }
}
