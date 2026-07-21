use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_file_with, binary_paths,
    hidden_home_parent, is_ide_extension_installed,
};
use crate::agents::object::AssetCore;
use crate::interfaces::{Asset, AssetType, ErasedAsset, MetaData};
use crate::utils::dir_exists;

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

impl ErasedAsset for MetaAsset {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn asset_type(&self) -> AssetType {
        AssetType::Meta
    }

    fn agent_name(&self) -> &str {
        self.core.agent_name()
    }

    fn agent_home(&self) -> &std::path::Path {
        self.core.agent_home()
    }

    fn data(&self) -> SentraResult<serde_json::Value> {
        serde_json::to_value(<Self as Asset<Option<MetaData>>>::get_data(self)?)
            .map_err(|err| crate::SentraError::Message(err.to_string()))
    }

    fn data_async<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = SentraResult<serde_json::Value>> + 'a>>
    {
        Box::pin(async move {
            serde_json::to_value(<Self as Asset<Option<MetaData>>>::get_data_async(self).await?)
                .map_err(|err| crate::SentraError::Message(err.to_string()))
        })
    }
}

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
    Ok(Some(MetaData {
        id: Some(agent_name.to_string()),
        name: agent_name.to_string(),
        description: Some(
            "Claude Code is Anthropic's AI-powered coding agent with skills, MCP, and scheduled task support."
                .to_string(),
        ),
        version: None,
        author: Some("Anthropic".to_string()),
        installed,
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
}

pub(super) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    if agent_name == crate::agents::entries::CLAUDE_CODE_IDE_AGENT_ENTRY.name {
        return is_ide_extension_installed(
            agent_home,
            crate::agents::claude_cli::CLAUDE_CODE_IDE_EXTENSION_ID,
        );
    }
    let probe = InstallStatusProbe::real();
    is_agent_installed_with(agent_home, &probe)
}

fn is_agent_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_command_exists_with(&["claude"], probe)
        || any_existing_file_with(claude_cli_install_paths(agent_home), probe)
        || probe.product_installed(&["Claude Code"], &["Anthropic"])
}

fn claude_cli_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "claude");
    paths.extend(binary_paths(
        user_home.join(".local").join("share").join("claude"),
        "claude",
    ));
    paths
}
