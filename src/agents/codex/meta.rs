use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_file_with, binary_paths, env_path,
    hidden_home_parent,
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
        let home = self.core.agent_home();
        let agent_name = self.core.agent_name();
        let installed = is_agent_installed(agent_name, home);
        if !dir_exists(home) && !installed {
            return Ok(None);
        }
        Ok(Some(MetaData {
            id: Some(agent_name.to_string()),
            name: agent_name.to_string(),
            description: Some(
                "Cloud-based AI coding agent by OpenAI that runs sandboxed tasks and writes, tests, and fixes code autonomously."
                    .to_string(),
            ),
            version: None,
            author: Some("OpenAI".to_string()),
            installed,
            home: Some(home.to_path_buf()),
            created_at: None,
            updated_at: None,
        }))
    }
}

pub(super) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real();
    is_agent_installed_with(agent_name, agent_home, &probe)
}

fn is_agent_installed_with(
    agent_name: &str,
    agent_home: &Path,
    probe: &InstallStatusProbe,
) -> bool {
    any_command_exists_with(&[agent_name], probe)
        || any_existing_file_with(codex_install_paths(agent_name, agent_home), probe)
}

fn codex_install_paths(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), agent_name);
    if let Some(local_app_data) = env_path("LOCALAPPDATA") {
        paths.extend(binary_paths(
            local_app_data
                .join("Programs")
                .join("OpenAI")
                .join("Codex")
                .join("bin"),
            agent_name,
        ));
    }
    if let Some(install_dir) = env_path("CODEX_INSTALL_DIR") {
        paths.extend(binary_paths(install_dir, agent_name));
    }
    paths
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::agents::codex::meta::is_agent_installed_with;
    use crate::agents::install_status::InstallStatusProbe;

    #[test]
    fn install_probe_accepts_command_presence() {
        let dir = tempfile::tempdir().unwrap();
        let codex_home = dir.path().join(".codex");
        let probe = InstallStatusProbe::test(
            only_codex_command_exists,
            path_never_exists,
            path_never_exists,
        );

        assert!(is_agent_installed_with("codex", &codex_home, &probe));
    }

    fn only_codex_command_exists(binary: &str) -> bool {
        binary == "codex"
    }

    fn path_never_exists(_: &Path) -> bool {
        false
    }
}
