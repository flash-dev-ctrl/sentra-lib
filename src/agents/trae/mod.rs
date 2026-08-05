use std::path::{Path, PathBuf};

use crate::interfaces::{AssetType, ErasedAsset};

mod cron;
mod install;
mod mcp;
mod memory;
mod meta;
mod plugin;
mod process;
mod provider;
mod scheduled_task_cloud;
mod scheduled_task_db;
mod skill;
mod surface;
mod vscode_plugin;
mod work_cron;
mod work_mcp;
mod work_memory;
mod work_meta;
mod work_provider;
mod work_skill;

pub(crate) use install::{
    cn_install_plans_for_platform, cn_uninstall_plans_for_platform,
    cn_work_install_plans_for_platform, cn_work_uninstall_plans_for_platform,
    install_plans_for_platform, uninstall_plans_for_platform, work_install_plans_for_platform,
    work_uninstall_plans_for_platform,
};

pub(crate) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    if surface::is_work(agent_name) {
        work_meta::is_agent_installed(agent_name, agent_home)
    } else if surface::is_vscode_plugin(agent_name) {
        vscode_plugin::is_agent_installed(agent_home)
    } else {
        meta::is_agent_installed(agent_name, agent_home)
    }
}

pub(crate) fn process_data() -> Vec<crate::interfaces::ProcessData> {
    process::process_data()
}

pub(crate) fn work_process_data() -> Vec<crate::interfaces::ProcessData> {
    process::work_process_data()
}

pub(crate) fn vscode_plugin_process_data() -> Vec<crate::interfaces::ProcessData> {
    process::vscode_plugin_process_data()
}

pub(crate) fn asset_for_type(
    agent_name: &str,
    agent_home: &Path,
    asset_type: AssetType,
) -> Vec<Box<dyn ErasedAsset>> {
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
                process::matcher(agent_name),
            ))],
            AssetType::Plugin => Vec::new(),
        };
    }
    if surface::is_vscode_plugin(agent_name) {
        return match asset_type {
            AssetType::Meta => vec![Box::new(vscode_plugin::MetaAsset::new(
                agent_name, agent_home,
            ))],
            AssetType::Plugin => vec![Box::new(vscode_plugin::PluginAsset::new(
                agent_name, agent_home,
            ))],
            AssetType::Process => vec![Box::new(crate::agents::process::ProcessAsset::new(
                agent_name,
                agent_home,
                process::matcher(agent_name),
            ))],
            _ => Vec::new(),
        };
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

fn workspace_path(path: impl AsRef<Path>) -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join(path.as_ref()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn trae_surfaces_route_supported_assets() {
        let home = Path::new("home");
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
                asset_for_type("trae-ide", home, asset_type).len(),
                1,
                "{asset_type:?}"
            );
        }

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
                asset_for_type("trae-work", home, asset_type).len(),
                1,
                "{asset_type:?}"
            );
        }
        assert!(asset_for_type("trae-work", home, AssetType::Plugin).is_empty());

        for asset_type in [AssetType::Meta, AssetType::Plugin, AssetType::Process] {
            assert_eq!(
                asset_for_type("trae-vscode-plugin", home, asset_type).len(),
                1,
                "{asset_type:?}"
            );
        }
        for asset_type in [
            AssetType::Skill,
            AssetType::Mcp,
            AssetType::Memory,
            AssetType::Cron,
            AssetType::Provider,
        ] {
            assert!(
                asset_for_type("trae-vscode-plugin", home, asset_type).is_empty(),
                "{asset_type:?}"
            );
        }
    }
}
