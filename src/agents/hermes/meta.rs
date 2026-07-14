use crate::SentraResult;
use crate::agents::install_status::{AgentInstallProbe, is_agent_installed};
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
    if !dir_exists(agent_home) {
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
        installed: is_agent_installed(AgentInstallProbe::Hermes, agent_home),
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
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
