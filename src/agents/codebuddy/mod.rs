use std::path::Path;

use crate::interfaces::{AssetType, ErasedAsset};

mod cron;
mod ide_cron;
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
mod work_memory;
mod work_meta;
mod work_process;
mod work_provider;
mod work_skill;

pub(crate) use install::{install_plans_for_platform, uninstall_plan_for_platform};
pub(crate) use work_install::{
    install_plans_for_platform as work_install_plans_for_platform,
    uninstall_plans_for_platform as work_uninstall_plans_for_platform,
};

pub(crate) fn discover_agents(user_home: impl AsRef<Path>) -> Vec<crate::agents::Agent> {
    let user_home = user_home.as_ref();
    let entries = crate::agents::entries::CODEBUDDY_AGENT_ENTRIES
        .iter()
        .filter(|entry| entry.name != crate::agents::entries::CODEBUDDY_IDE_PLUGIN_AGENT_ENTRY.name)
        .cloned()
        .collect::<Vec<_>>();
    let mut agents = crate::agents::discovery::discover_entry_agents(user_home, &entries);
    let default_cli_home = user_home.join(surface::cli_home_dir(
        crate::agents::entries::CODEBUDDY_CLI_AGENT_ENTRY.name,
    ));
    if meta::is_agent_installed(
        crate::agents::entries::CODEBUDDY_IDE_PLUGIN_AGENT_ENTRY.name,
        &default_cli_home,
    ) {
        let mut plugin_homes = agents
            .iter()
            .filter(|agent| agent.name() == crate::agents::entries::CODEBUDDY_CLI_AGENT_ENTRY.name)
            .map(|agent| agent.home().to_path_buf())
            .collect::<Vec<_>>();
        if plugin_homes.is_empty() {
            plugin_homes.push(default_cli_home);
        }
        for home in plugin_homes {
            agents.push(crate::agents::Agent::new(
                &crate::agents::entries::CODEBUDDY_IDE_PLUGIN_AGENT_ENTRY,
                home,
            ));
        }
    }
    agents
}

pub(crate) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    if surface::is_work(agent_name) {
        work_meta::is_agent_installed(agent_name, agent_home)
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

pub(crate) fn plugin_process_data() -> Vec<crate::interfaces::ProcessData> {
    process::plugin_process_data()
}

pub(crate) fn work_process_data() -> Vec<crate::interfaces::ProcessData> {
    work_process::process_data()
}

pub(crate) fn asset_for_type(
    agent_name: &str,
    agent_home: &Path,
    asset_type: AssetType,
) -> Vec<Box<dyn ErasedAsset>> {
    if surface::is_ide_plugin(agent_name) {
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
    if surface::is_ide(agent_name) {
        return match asset_type {
            AssetType::Meta => vec![Box::new(meta::MetaAsset::new(agent_name, agent_home))],
            AssetType::Cron => vec![Box::new(ide_cron::CronAsset::new(agent_name, agent_home))],
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
            AssetType::Memory => vec![Box::new(work_memory::MemoryAsset::new(
                agent_name, agent_home,
            ))],
            AssetType::Cron => vec![Box::new(work_cron::CronAsset::new(agent_name, agent_home))],
            AssetType::Provider => vec![Box::new(work_provider::ProviderAsset::new(
                agent_name, agent_home,
            ))],
            AssetType::Process => vec![Box::new(crate::agents::process::ProcessAsset::new(
                agent_name,
                agent_home,
                work_process::matches_process,
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
            AssetType::Provider => vec![Box::new(provider::ProviderAsset::new(
                agent_name, agent_home,
            ))],
            AssetType::Plugin => vec![Box::new(plugin::PluginAsset::new(agent_name, agent_home))],
            AssetType::Process => vec![Box::new(crate::agents::process::ProcessAsset::new(
                agent_name,
                agent_home,
                process::matcher(agent_name),
            ))],
            AssetType::Cron => Vec::new(),
        };
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn codebuddy_surfaces_route_only_supported_assets() {
        let home = Path::new("home");
        for asset_type in [
            AssetType::Meta,
            AssetType::Skill,
            AssetType::Mcp,
            AssetType::Memory,
            AssetType::Provider,
            AssetType::Plugin,
            AssetType::Process,
        ] {
            assert_eq!(
                asset_for_type("codebuddy-cli", home, asset_type).len(),
                1,
                "{asset_type:?}"
            );
        }
        assert!(asset_for_type("codebuddy-cli", home, AssetType::Cron).is_empty());

        for ide in ["codebuddy-ide", "codebuddy-cn-ide"] {
            for asset_type in [AssetType::Meta, AssetType::Cron, AssetType::Process] {
                assert_eq!(asset_for_type(ide, home, asset_type).len(), 1, "{ide}");
            }
            for asset_type in [
                AssetType::Skill,
                AssetType::Mcp,
                AssetType::Memory,
                AssetType::Provider,
                AssetType::Plugin,
            ] {
                assert!(asset_for_type(ide, home, asset_type).is_empty(), "{ide}");
            }
        }

        for asset_type in [AssetType::Meta, AssetType::Process] {
            assert_eq!(
                asset_for_type("codebuddy-ide-plugin", home, asset_type).len(),
                1
            );
        }
        assert!(asset_for_type("codebuddy-ide-plugin", home, AssetType::Mcp).is_empty());

        for asset_type in [
            AssetType::Meta,
            AssetType::Skill,
            AssetType::Mcp,
            AssetType::Memory,
            AssetType::Cron,
            AssetType::Provider,
            AssetType::Process,
        ] {
            assert_eq!(
                asset_for_type("workbuddy", home, asset_type).len(),
                1,
                "{asset_type:?}"
            );
        }
        assert!(asset_for_type("workbuddy", home, AssetType::Plugin).is_empty());
    }
}
