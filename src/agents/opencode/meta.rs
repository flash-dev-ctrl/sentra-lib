use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_file_with, binary_paths,
    hidden_home_parent,
};
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
    let installed = is_agent_installed(agent_name, agent_home);
    if !dir_exists(agent_home)
        && !crate::agents::opencode::config_files(agent_home)
            .iter()
            .any(|path| path.is_file())
        && !dir_exists(&data_home)
        && !installed
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
        installed,
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
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
        || any_existing_file_with(opencode_install_paths(agent_name, agent_home), probe)
}

fn opencode_install_paths(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let user_home = opencode_user_home(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), agent_name);
    paths.extend(binary_paths(agent_home.join("bin"), agent_name));
    paths
}

fn opencode_user_home(agent_home: &Path) -> PathBuf {
    if agent_home.file_name().and_then(|name| name.to_str()) == Some("opencode")
        && agent_home
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some(".config")
    {
        return agent_home
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| agent_home.to_path_buf());
    }
    hidden_home_parent(agent_home)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::agents::install_status::InstallStatusProbe;
    use crate::agents::opencode::meta::is_agent_installed_with;

    #[test]
    fn install_probe_requires_binary_or_install_path_not_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        let opencode_home = dir.path().join(".config").join("opencode");
        std::fs::create_dir_all(&opencode_home).unwrap();
        let probe =
            InstallStatusProbe::test(command_never_exists, path_never_exists, path_never_exists);

        assert!(!is_agent_installed_with("opencode", &opencode_home, &probe));
    }

    #[test]
    fn install_probe_accepts_known_user_install_path() {
        let dir = tempfile::tempdir().unwrap();
        let opencode_home = dir.path().join(".config").join("opencode");
        let bin_dir = dir.path().join(".local").join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join(test_binary_name("opencode")), "").unwrap();
        let probe = InstallStatusProbe::test(command_never_exists, path_is_file, path_never_exists);

        assert!(is_agent_installed_with("opencode", &opencode_home, &probe));
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
