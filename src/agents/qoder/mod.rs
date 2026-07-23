use std::path::Path;

use crate::interfaces::{AssetType, ErasedAsset};

mod cron;
mod install;
mod mcp;
mod memory;
mod meta;
mod plugin;
mod process;
mod provider;
mod skill;
mod surface;
mod work_cron;
mod work_install;
mod work_mcp;
mod work_meta;
mod work_process;
mod work_skill;

pub(crate) use install::{install_plans_for_platform, uninstall_plans_for_platform};
pub(crate) use work_install::{
    install_plans_for_platform as work_install_plans_for_platform,
    uninstall_plans_for_platform as work_uninstall_plans_for_platform,
};

pub(crate) fn discover_agents(user_home: impl AsRef<Path>) -> Vec<crate::agents::Agent> {
    crate::agents::discovery::discover_entry_agents(
        user_home.as_ref(),
        crate::agents::entries::QODER_AGENT_ENTRIES,
    )
}

pub(crate) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    if surface::is_work(agent_name) {
        work_meta::is_agent_installed(agent_name, agent_home)
    } else {
        meta::is_agent_installed(agent_name, agent_home)
    }
}

pub(crate) fn is_install_target_installed(agent_home: &Path) -> bool {
    meta::is_install_target_installed(agent_home)
}

pub(crate) fn process_data() -> Vec<crate::interfaces::ProcessData> {
    process::process_data()
}

pub(crate) fn work_process_data() -> Vec<crate::interfaces::ProcessData> {
    work_process::process_data()
}

pub(crate) fn asset_for_type(
    agent_name: &str,
    agent_home: &Path,
    asset_type: AssetType,
) -> Vec<Box<dyn ErasedAsset>> {
    if surface::is_ide(agent_name) {
        return match asset_type {
            AssetType::Meta => vec![Box::new(meta::MetaAsset::new(agent_name, agent_home))],
            AssetType::Process => vec![Box::new(crate::agents::process::ProcessAsset::new(
                agent_name,
                agent_home,
                process::matcher(agent_name),
            ))],
            _ => Vec::new(),
        };
    }
    if surface::is_work(agent_name) {
        return match asset_type {
            AssetType::Meta => vec![Box::new(work_meta::MetaAsset::new(agent_name, agent_home))],
            AssetType::Skill => vec![Box::new(work_skill::SkillAsset::new(
                agent_name, agent_home,
            ))],
            AssetType::Mcp => vec![Box::new(work_mcp::McpAsset::new(agent_name, agent_home))],
            AssetType::Cron => vec![Box::new(work_cron::CronAsset::new(agent_name, agent_home))],
            AssetType::Process => vec![Box::new(crate::agents::process::ProcessAsset::new(
                agent_name,
                agent_home,
                work_process::matcher(agent_name),
            ))],
            _ => Vec::new(),
        };
    }
    if surface::is_cli(agent_name) {
        return match asset_type {
            AssetType::Meta => vec![Box::new(meta::MetaAsset::new(agent_name, agent_home))],
            AssetType::Skill => vec![Box::new(skill::SkillAsset::new(agent_name, agent_home))],
            AssetType::Mcp => vec![Box::new(mcp::McpAsset::new(agent_name, agent_home))],
            AssetType::Memory => vec![Box::new(memory::MemoryAsset::new(agent_name, agent_home))],
            AssetType::Cron => vec![Box::new(cron::CronAsset::new(agent_name, agent_home))],
            AssetType::Provider => vec![Box::new(provider::ProviderAsset::new(
                agent_name, agent_home,
            ))],
            AssetType::Plugin => vec![Box::new(plugin::PluginAsset::new(agent_name, agent_home))],
            AssetType::Process => vec![Box::new(crate::agents::process::ProcessAsset::new(
                agent_name,
                agent_home,
                process::matcher(agent_name),
            ))],
        };
    }
    Vec::new()
}
