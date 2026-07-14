use crate::SentraResult;
use crate::agents::install_status::{AgentInstallProbe, is_agent_installed};
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, MetaData};
use crate::utils::{dir_exists, read_json_file};

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
    let data_home = crate::agents::opencode::data_home(agent_home);
    if !dir_exists(agent_home)
        && !crate::agents::opencode::config_files(agent_home)
            .iter()
            .any(|path| path.is_file())
        && !dir_exists(&data_home)
    {
        return Ok(None);
    }
    let mut config = serde_json::Value::Null;
    for path in crate::agents::opencode::config_files(agent_home) {
        if let Some(value) = read_json_file(path)? {
            config = value;
            break;
        }
    }
    Ok(Some(MetaData {
        id: Some(agent_name.to_string()),
        name: config
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("OpenCode")
            .to_string(),
        description: Some(
            "OpenCode is an AI coding agent with configurable providers, MCP servers, skills, agents, and commands."
                .to_string(),
        ),
        version: config
            .get("version")
            .and_then(|value| value.as_str())
            .or_else(|| {
                config
                    .get("meta")
                    .and_then(|meta| meta.get("version"))
                    .and_then(|value| value.as_str())
            })
            .map(str::to_string),
        author: Some("OpenCode".to_string()),
        installed: is_agent_installed(AgentInstallProbe::OpenCode, agent_home),
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
}
