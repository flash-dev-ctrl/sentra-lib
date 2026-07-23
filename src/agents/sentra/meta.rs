use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_file_with, binary_paths,
    hidden_home_parent,
};
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, MetaData};
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
    Ok(Some(MetaData {
        id: Some(agent_name.to_string()),
        name: "Sentra".to_string(),
        description: Some("Local AI agent asset scanner and manager.".to_string()),
        version: None,
        author: Some("Chaitin".to_string()),
        installed,
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
}

pub(super) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real(hidden_home_parent(agent_home));
    is_agent_installed_with(agent_name, agent_home, &probe)
}

fn is_agent_installed_with(
    agent_name: &str,
    agent_home: &Path,
    probe: &InstallStatusProbe,
) -> bool {
    any_command_exists_with(&[agent_name], probe)
        || any_existing_file_with(sentra_install_paths(agent_name, agent_home), probe)
}

fn sentra_install_paths(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(agent_home.join("bin"), agent_name);
    paths.extend(binary_paths(
        user_home.join(".local").join("bin"),
        agent_name,
    ));
    paths
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::agents::install_status::InstallStatusProbe;
    use crate::agents::sentra::meta::is_agent_installed_with;

    #[test]
    fn install_probe_accepts_agent_home_bin() {
        let dir = tempfile::tempdir().unwrap();
        let sentra_home = dir.path().join(".sentra");
        let bin_dir = sentra_home.join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join(test_binary_name("sentra")), "").unwrap();
        let probe = InstallStatusProbe::test(command_never_exists, path_is_file, path_never_exists);

        assert!(is_agent_installed_with("sentra", &sentra_home, &probe));
    }

    fn command_never_exists(_: &str) -> bool {
        false
    }

    fn path_never_exists(_: &Path) -> bool {
        false
    }

    fn path_is_file(path: &Path) -> bool {
        path.is_file()
    }

    fn test_binary_name(binary: &str) -> String {
        if cfg!(windows) {
            format!("{binary}.exe")
        } else {
            binary.to_string()
        }
    }
}
