use crate::SentraResult;
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
    if !dir_exists(agent_home) {
        return Ok(None);
    }
    let config = read_json_file(agent_home.join("openclaw.json"))?.unwrap_or_default();
    let meta = config.get("meta").and_then(|value| value.as_object());
    let wizard = config.get("wizard").and_then(|value| value.as_object());
    Ok(Some(MetaData {
        id: config
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .or_else(|| Some(agent_name.to_string())),
        name: config
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("OpenClaw")
            .to_string(),
        description: Some(
            "OpenClaw is an AI-powered automation agent for computer tasks.".to_string(),
        ),
        version: meta
            .and_then(|meta| meta.get("lastTouchedVersion"))
            .and_then(|value| value.as_str())
            .or_else(|| config.get("version").and_then(|value| value.as_str()))
            .map(str::to_string),
        author: config
            .get("author")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        home: Some(agent_home.to_path_buf()),
        created_at: wizard
            .and_then(|wizard| wizard.get("lastRunAt"))
            .and_then(|value| value.as_str())
            .or_else(|| config.get("createdAt").and_then(|value| value.as_str()))
            .map(str::to_string),
        updated_at: meta
            .and_then(|meta| meta.get("lastTouchedAt"))
            .and_then(|value| value.as_str())
            .or_else(|| config.get("updatedAt").and_then(|value| value.as_str()))
            .map(str::to_string),
    }))
}
