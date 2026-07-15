use crate::SentraResult;
use crate::agents::discovery::get_agent_title;
use crate::agents::install_status::{InstallStatusProbe, is_named_cli_agent_installed_with};
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
        name: get_agent_title(agent_name),
        description: None,
        version: None,
        author: None,
        installed,
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
}

pub(super) fn is_agent_installed(agent_name: &str, agent_home: &std::path::Path) -> bool {
    let probe = InstallStatusProbe::real();
    is_agent_installed_with(agent_name, agent_home, &probe)
}

fn is_agent_installed_with(
    agent_name: &str,
    agent_home: &std::path::Path,
    probe: &InstallStatusProbe,
) -> bool {
    is_named_cli_agent_installed_with(agent_name, agent_home, probe)
}

#[cfg(test)]
mod tests {
    use crate::agents::general::meta::is_agent_installed_with;
    use crate::agents::install_status::InstallStatusProbe;

    #[test]
    fn copilot_uses_copilot_cli_command() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".copilot");
        let probe = InstallStatusProbe::test(
            only_copilot_command_exists,
            path_never_exists,
            path_never_exists,
        );

        assert!(is_agent_installed_with("copilot", &agent_home, &probe));
    }

    fn only_copilot_command_exists(binary: &str) -> bool {
        binary == "copilot"
    }

    fn path_never_exists(_: &std::path::Path) -> bool {
        false
    }
}
