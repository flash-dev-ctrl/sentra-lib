use std::path::Path;

use crate::interfaces::{AssetType, ErasedAsset};

mod install;
mod mcp;
mod meta;
mod process;

pub(crate) use install::{install_plans_for_platform, uninstall_plans_for_platform};
mod skill;

pub(crate) fn discover_agents(user_home: impl AsRef<Path>) -> Vec<crate::agents::Agent> {
    crate::agents::discovery::discover_entry_agents(
        user_home.as_ref(),
        std::slice::from_ref(&crate::agents::entries::WORKBUDDY_AGENT_ENTRY),
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
        AssetType::Process => vec![Box::new(crate::agents::process::ProcessAsset::new(
            agent_name,
            agent_home,
            process::matches_process,
        ))],
        _ => Vec::new(),
    }
}
