use std::path::Path;

use crate::interfaces::{AssetType, ErasedAsset};

mod app_cron;
mod app_mcp;
mod app_memory;
mod app_meta;
mod app_process;
mod app_provider;
mod app_skill;
mod cron;
mod install;
mod mcp;
mod memory;
mod meta;
mod plugin;
mod process;
mod provider;
mod skill;

pub(crate) use install::{install_plans_for_platform, uninstall_plans_for_platform};

pub(crate) const CLAUDE_CODE_IDE_EXTENSION_ID: &str = "anthropic.claude-code";

pub(crate) fn discover_agents(user_home: impl AsRef<Path>) -> Vec<crate::agents::Agent> {
    let user_home = user_home.as_ref();
    let mut agents = crate::agents::discovery::discover_entry_agents(
        user_home,
        std::slice::from_ref(&crate::agents::entries::CLAUDE_CLI_AGENT_ENTRY),
    );
    agents.extend(crate::agents::discovery::discover_installed_entry_agents(
        user_home,
        &[&crate::agents::entries::CLAUDE_CLI_IDE_AGENT_ENTRY],
    ));
    agents.extend(crate::agents::discovery::discover_entry_agents(
        user_home,
        std::slice::from_ref(&crate::agents::entries::CLAUDE_APP_AGENT_ENTRY),
    ));
    agents
}

pub(crate) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    if agent_name == crate::agents::entries::CLAUDE_APP_AGENT_ENTRY.name {
        app_meta::is_agent_installed(agent_name, agent_home)
    } else {
        meta::is_agent_installed(agent_name, agent_home)
    }
}

pub(crate) fn process_data() -> Vec<crate::interfaces::ProcessData> {
    process::process_data()
}

pub(crate) fn ide_process_data() -> Vec<crate::interfaces::ProcessData> {
    process::ide_process_data()
}

pub(crate) fn app_process_data() -> Vec<crate::interfaces::ProcessData> {
    app_process::process_data()
}

pub(crate) fn asset_for_type(
    agent_name: &str,
    agent_home: &std::path::Path,
    asset_type: AssetType,
) -> Vec<Box<dyn ErasedAsset>> {
    if agent_name == crate::agents::entries::CLAUDE_APP_AGENT_ENTRY.name {
        return app_asset_for_type(agent_name, agent_home, asset_type);
    }
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

fn app_asset_for_type(
    agent_name: &str,
    agent_home: &Path,
    asset_type: AssetType,
) -> Vec<Box<dyn ErasedAsset>> {
    match asset_type {
        AssetType::Meta => vec![Box::new(app_meta::MetaAsset::new(agent_name, agent_home))],
        AssetType::Skill => vec![Box::new(app_skill::SkillAsset::new(agent_name, agent_home))],
        AssetType::Mcp => vec![Box::new(app_mcp::McpAsset::new(agent_name, agent_home))],
        AssetType::Cron => vec![Box::new(app_cron::CronAsset::new(agent_name, agent_home))],
        AssetType::Provider => vec![Box::new(app_provider::ProviderAsset::new(
            agent_name, agent_home,
        ))],
        AssetType::Memory | AssetType::Plugin => Vec::new(),
        AssetType::Process => vec![Box::new(crate::agents::process::ProcessAsset::new(
            agent_name,
            agent_home,
            app_process::matches_process,
        ))],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ide_extension_shares_claude_assets() {
        let home = Path::new(".claude");

        assert_eq!(
            asset_for_type("claude-cli-ide", home, AssetType::Meta).len(),
            1
        );
        assert_eq!(
            asset_for_type("claude-cli-ide", home, AssetType::Process).len(),
            1
        );
        assert_eq!(
            asset_for_type("claude-cli-ide", home, AssetType::Skill).len(),
            1
        );
        assert_eq!(
            asset_for_type("claude-cli-ide", home, AssetType::Mcp).len(),
            1
        );
        assert_eq!(
            asset_for_type("claude-cli-ide", home, AssetType::Provider).len(),
            1
        );
        assert_eq!(
            asset_for_type("claude-cli-ide", home, AssetType::Cron).len(),
            1
        );
        assert_eq!(
            asset_for_type("claude-cli-ide", home, AssetType::Plugin).len(),
            1
        );
        assert_eq!(
            asset_for_type("claude-cli-ide", home, AssetType::Memory).len(),
            1
        );
    }

    #[test]
    fn desktop_app_routes_app_specific_assets() {
        let home = Path::new("Claude");

        for asset_type in [
            AssetType::Meta,
            AssetType::Skill,
            AssetType::Mcp,
            AssetType::Cron,
            AssetType::Provider,
            AssetType::Process,
        ] {
            assert_eq!(asset_for_type("claude-app", home, asset_type).len(), 1);
        }
        assert!(asset_for_type("claude-app", home, AssetType::Memory).is_empty());
        assert!(asset_for_type("claude-app", home, AssetType::Plugin).is_empty());
    }
}
