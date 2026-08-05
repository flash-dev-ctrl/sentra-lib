use std::path::{Path, PathBuf};

use crate::interfaces::{AssetType, ErasedAsset};

mod app_cron;
mod app_mcp;
mod app_memory;
mod app_plugin;
mod app_provider;
mod app_skill;
mod cron;
mod install;
mod mcp;
mod meta;
mod plugin;
mod process;
mod provider;
mod skill;

pub(crate) use install::{install_plans_for_platform, uninstall_plan_for_platform};

pub(crate) const KIMI_CODE_IDE_EXTENSION_ID: &str = "moonshot-ai.kimi-code";

pub(crate) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    meta::is_agent_installed(agent_name, agent_home)
}

pub(crate) fn process_data() -> Vec<crate::interfaces::ProcessData> {
    process::process_data()
}

pub(crate) fn app_process_data() -> Vec<crate::interfaces::ProcessData> {
    process::app_process_data()
}

pub(crate) fn ide_process_data() -> Vec<crate::interfaces::ProcessData> {
    process::ide_process_data()
}

pub(super) fn app_daimon_home(agent_home: &Path) -> PathBuf {
    agent_home.join("daimon-share").join("daimon")
}

pub(super) fn app_runtime_home(agent_home: &Path) -> PathBuf {
    app_daimon_home(agent_home)
        .join("runtime")
        .join("kimi-code")
        .join("home")
}

pub(crate) fn asset_for_type(
    agent_name: &str,
    agent_home: &std::path::Path,
    asset_type: AssetType,
) -> Vec<Box<dyn ErasedAsset>> {
    if agent_name == crate::agents::entries::KIMI_APP_AGENT_ENTRY.name {
        return app_asset_for_type(agent_name, agent_home, asset_type);
    }
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
            process::matcher(agent_name),
        ))],
    }
}

fn app_asset_for_type(
    agent_name: &str,
    agent_home: &Path,
    asset_type: AssetType,
) -> Vec<Box<dyn ErasedAsset>> {
    match asset_type {
        AssetType::Meta => vec![Box::new(meta::MetaAsset::new(agent_name, agent_home))],
        AssetType::Skill => vec![Box::new(app_skill::SkillAsset::new(agent_name, agent_home))],
        AssetType::Mcp => vec![Box::new(app_mcp::McpAsset::new(agent_name, agent_home))],
        AssetType::Memory => vec![Box::new(app_memory::MemoryAsset::new(
            agent_name, agent_home,
        ))],
        AssetType::Cron => vec![Box::new(app_cron::CronAsset::new(agent_name, agent_home))],
        AssetType::Provider => vec![Box::new(app_provider::ProviderAsset::new(
            agent_name, agent_home,
        ))],
        AssetType::Plugin => vec![Box::new(app_plugin::PluginAsset::new(
            agent_name, agent_home,
        ))],
        AssetType::Process => vec![Box::new(crate::agents::process::ProcessAsset::new(
            agent_name,
            agent_home,
            process::matcher(agent_name),
        ))],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kimi_surfaces_route_supported_assets() {
        let cli_home = Path::new(".kimi-code");
        for agent_name in ["kimi-cli", "kimi-cli-ide"] {
            for asset_type in [
                AssetType::Meta,
                AssetType::Skill,
                AssetType::Mcp,
                AssetType::Cron,
                AssetType::Provider,
                AssetType::Plugin,
                AssetType::Process,
            ] {
                assert_eq!(asset_for_type(agent_name, cli_home, asset_type).len(), 1);
            }
            assert!(asset_for_type(agent_name, cli_home, AssetType::Memory).is_empty());
        }

        for asset_type in [
            AssetType::Meta,
            AssetType::Skill,
            AssetType::Mcp,
            AssetType::Memory,
            AssetType::Cron,
            AssetType::Provider,
            AssetType::Plugin,
            AssetType::Process,
        ] {
            assert_eq!(
                asset_for_type("kimi-app", Path::new("kimi-desktop"), asset_type).len(),
                1
            );
        }
    }
}
