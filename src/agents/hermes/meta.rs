use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_file_with, binary_paths,
    hidden_home_parent,
};
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, MetaData};
use crate::utils::{dir_exists, read_text_file};

#[derive(Debug, Clone)]
pub(super) struct MetaAsset {
    pub(crate) core: AssetCore,
}

impl MetaAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(MetaAsset, AssetType::Meta, Option<MetaData>);

impl Asset<Option<MetaData>> for MetaAsset {
    fn get_data(&self) -> SentraResult<Option<MetaData>> {
        meta_data(self.core.agent_name(), self.core.agent_home())
    }
}

fn meta_data(agent_name: &str, agent_home: &std::path::Path) -> SentraResult<Option<MetaData>> {
    let installed = is_agent_installed(agent_name, agent_home);
    if !dir_exists(agent_home) && !installed {
        return Ok(None);
    }
    let config = read_yaml_file(&agent_home.join("config.yaml"))?;
    let version = config
        .as_ref()
        .and_then(|value| value.get("_config_version"))
        .and_then(scalar_string);
    Ok(Some(MetaData {
        id: Some(agent_name.to_string()),
        name: "Hermes".to_string(),
        description: Some(
            "The self-improving AI agent that creates and improves skills from experience."
                .to_string(),
        ),
        version,
        author: Some("Nous Research".to_string()),
        installed,
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
}

pub(super) fn is_agent_installed(_agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real();
    is_agent_installed_with(agent_home, &probe)
}

fn is_agent_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_command_exists_with(&["hermes", "hermes-agent"], probe)
        || any_existing_file_with(hermes_install_paths(agent_home), probe)
}

fn hermes_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "hermes");
    paths.extend(binary_paths(
        user_home.join(".local").join("bin"),
        "hermes-agent",
    ));
    paths.extend(binary_paths(agent_home.join("bin"), "hermes"));
    paths.extend(binary_paths(agent_home.join("bin"), "hermes-agent"));
    paths.extend(binary_paths("/usr/local/bin", "hermes"));
    paths.extend(binary_paths("/usr/local/bin", "hermes-agent"));
    paths
}

fn read_yaml_file(path: &std::path::Path) -> SentraResult<Option<serde_yaml::Value>> {
    let Some(content) = read_text_file(path)? else {
        return Ok(None);
    };
    serde_yaml::from_str(&content).map(Some).map_err(Into::into)
}

fn scalar_string(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::String(value) => Some(value.clone()),
        serde_yaml::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}
