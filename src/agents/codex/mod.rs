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

pub(crate) use install::{install_plans_for_platform, uninstall_plan_for_platform};

pub(crate) const CODEX_IDE_EXTENSION_ID: &str = "openai.chatgpt";

pub(crate) fn discover_agents(user_home: impl AsRef<Path>) -> Vec<crate::agents::Agent> {
    let user_home = user_home.as_ref();
    let mut agents = crate::agents::discovery::discover_entry_agents(
        user_home,
        std::slice::from_ref(&crate::agents::entries::CODEX_CLI_AGENT_ENTRY),
    );
    agents.extend(crate::agents::discovery::discover_installed_entry_agents(
        user_home,
        &[
            &crate::agents::entries::CODEX_APP_AGENT_ENTRY,
            &crate::agents::entries::CODEX_CLI_IDE_AGENT_ENTRY,
        ],
    ));
    agents
}

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

pub(crate) fn asset_for_type(
    agent_name: &str,
    agent_home: &std::path::Path,
    asset_type: AssetType,
) -> Vec<Box<dyn ErasedAsset>> {
    match asset_type {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ide_extension_uses_shared_asset_factories() {
        let home = Path::new(".codex");

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
            assert_eq!(asset_for_type("codex-cli-ide", home, asset_type).len(), 1);
        }
    }

    #[test]
    fn desktop_app_uses_shared_asset_factories() {
        let home = Path::new(".codex");

        for agent_name in ["codex-cli", "codex-app"] {
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
                assert_eq!(asset_for_type(agent_name, home, asset_type).len(), 1);
            }
        }
    }
}
