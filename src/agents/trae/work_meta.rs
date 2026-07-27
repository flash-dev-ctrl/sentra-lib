use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_dir_with, any_existing_file_with,
    binary_paths, env_path,
};
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::agents::trae::surface;
use crate::interfaces::{Asset, AssetType, MetaData};
use crate::utils::dir_exists;

#[derive(Debug, Clone)]
pub(super) struct MetaAsset {
    core: AssetCore,
}

impl MetaAsset {
    pub(super) fn new(agent_name: impl Into<String>, agent_home: impl Into<PathBuf>) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(MetaAsset, AssetType::Meta, Option<MetaData>);

impl Asset<Option<MetaData>> for MetaAsset {
    fn get_data(&self) -> SentraResult<Option<MetaData>> {
        let home = self.core.agent_home();
        let installed = is_agent_installed(self.core.agent_name(), home);
        if !dir_exists(home) && !installed {
            return Ok(None);
        }
        Ok(Some(MetaData {
            id: Some(self.core.agent_name().to_string()),
            name: surface::title(self.core.agent_name()).to_string(),
            description: Some(
                "Trae Work local workbench configuration and Agent assets.".to_string(),
            ),
            version: None,
            author: Some(surface::author(self.core.agent_name()).to_string()),
            installed,
            home: Some(home.to_path_buf()),
            ..MetaData::default()
        }))
    }
}

pub(super) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    let user_home = surface::user_home(agent_name, agent_home);
    let probe = InstallStatusProbe::real(&user_home);
    let commands: &[&str] = if surface::is_cn(agent_name) {
        &["traeworkcn", "TraeWorkCN", "Trae CN Work", "Trae Work CN"]
    } else {
        &["traework", "TraeWork", "Trae Work", "TRAE SOLO"]
    };
    any_command_exists_with(commands, &probe)
        || any_existing_file_with(install_paths(agent_name, agent_home), &probe)
        || any_existing_dir_with(work_state_paths(agent_name, agent_home), &probe)
        || any_existing_dir_with(app_paths(agent_name, agent_home), &probe)
        || probe.product_installed(product_names(agent_name), publishers(agent_name))
}

fn install_paths(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let user_home = surface::user_home(agent_name, agent_home);
    let command = if surface::is_cn(agent_name) {
        "TraeWorkCN"
    } else {
        "TraeWork"
    };
    let mut paths = binary_paths(user_home.join(".local").join("bin"), command);
    if let Some(local_app_data) = env_path("LOCALAPPDATA") {
        for app_name in surface::work_app_names(agent_name) {
            paths.extend(binary_paths(
                local_app_data.join("Programs").join(app_name),
                command,
            ));
        }
    }
    paths
}

fn app_paths(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let user_home = surface::user_home(agent_name, agent_home);
    let mut paths = Vec::new();
    for app_name in surface::work_app_names(agent_name) {
        paths.extend([
            user_home
                .join("Applications")
                .join(format!("{app_name}.app")),
            PathBuf::from("/Applications").join(format!("{app_name}.app")),
        ]);
    }
    paths.extend(surface::work_data_roots(agent_name, agent_home));
    paths
}

fn work_state_paths(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let state_home = surface::state_home(agent_name, agent_home);
    vec![
        state_home.join("work"),
        state_home.join("worktrees"),
        state_home.join("builtin").join("work"),
    ]
}

fn product_names(agent_name: &str) -> &'static [&'static str] {
    if surface::is_cn(agent_name) {
        &[
            "Trae CN Work",
            "Trae Work CN",
            "TRAE Work CN",
            "TRAE SOLO CN",
        ]
    } else {
        &["Trae Work", "TRAE Work", "TraeWork", "TRAE SOLO"]
    }
}

fn publishers(agent_name: &str) -> &'static [&'static str] {
    if surface::is_cn(agent_name) {
        &["Beijing Yinli", "北京引力", "ByteDance"]
    } else {
        &["SPRING (SG)", "ByteDance"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_state_path_counts_as_work_installation() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".trae").join("work");
        std::fs::create_dir_all(&home).unwrap();

        assert!(is_agent_installed("trae-work", &home));
    }
}
