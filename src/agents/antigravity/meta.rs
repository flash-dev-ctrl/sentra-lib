use std::path::{Path, PathBuf};

use crate::agents::install_status::{
    any_command_exists_with, any_existing_dir_with, any_existing_file_with, binary_paths,
    hidden_home_parent, InstallStatusProbe,
};
use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{Asset, AssetType, MetaData};
use crate::utils::dir_exists;
use crate::SentraResult;

#[derive(Debug, Clone)]
pub(super) struct MetaAsset {
    pub(crate) core: AssetCore,
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
            name: self.core.agent_name().to_string(),
            description: Some("Google Antigravity agent CLI configuration.".to_string()),
            version: None,
            author: Some("Google".to_string()),
            installed,
            home: Some(home.to_path_buf()),
            created_at: None,
            updated_at: None,
        }))
    }
}

pub(super) fn is_agent_installed(_agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real();
    is_agent_installed_with(agent_home, &probe)
}

fn is_agent_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_command_exists_with(&["agy", "Antigravity"], probe)
        || any_existing_file_with(binary_paths(agent_home.join("bin"), "agy"), probe)
        || any_existing_dir_with(
            vec![agent_home.to_path_buf(), antigravity_home(agent_home)],
            probe,
        )
}

fn antigravity_home(agent_home: &Path) -> PathBuf {
    let home = agent_home
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| hidden_home_parent(agent_home));
    home.join(".gemini").join("antigravity-cli")
}

#[cfg(test)]
mod tests {
    use super::is_agent_installed_with;
    use crate::agents::install_status::InstallStatusProbe;

    #[test]
    fn install_probe_accepts_agy_command() {
        let dir = tempfile::tempdir().unwrap();
        let probe = InstallStatusProbe::test(|binary| binary == "agy", |_| false, |_| false);

        assert!(is_agent_installed_with(dir.path(), &probe));
    }
}
