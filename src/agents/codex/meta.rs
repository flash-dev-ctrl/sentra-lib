use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_dir_with, any_existing_file_with,
    binary_paths, env_path, hidden_home_parent, is_ide_extension_installed,
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
    if agent_name == crate::agents::entries::CODEX_IDE_AGENT_ENTRY.name {
        return is_ide_extension_installed(
            agent_home,
            crate::agents::codex::CODEX_IDE_EXTENSION_ID,
        );
    }
    let probe = InstallStatusProbe::real();
    if agent_name == crate::agents::entries::CODEX_APP_AGENT_ENTRY.name {
        is_codex_app_installed_with(agent_home, &probe)
    } else {
        is_codex_cli_installed_with(agent_name, agent_home, &probe)
    }
}

fn is_codex_cli_installed_with(
    agent_name: &str,
    agent_home: &Path,
    probe: &InstallStatusProbe,
) -> bool {
    any_command_exists_with(&[agent_name], probe)
        || any_existing_file_with(codex_install_paths(agent_name, agent_home), probe)
}

fn is_codex_app_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_existing_file_with(codex_app_executable_paths(agent_home), probe)
        || any_existing_dir_with(codex_app_dir_paths(agent_home), probe)
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

fn codex_app_executable_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = Vec::new();

    for dir in windows_app_roots(&user_home) {
        paths.push(dir.join("Codex.exe"));
        paths.push(dir.join("ChatGPT.exe"));
        paths.push(dir.join("OpenAI").join("Codex").join("Codex.exe"));
        paths.push(dir.join("OpenAI").join("ChatGPT").join("ChatGPT.exe"));
        paths.push(dir.join("OpenAI Codex").join("Codex.exe"));
        paths.push(dir.join("OpenAI ChatGPT").join("ChatGPT.exe"));
        paths.push(dir.join("Codex").join("Codex.exe"));
        paths.push(dir.join("ChatGPT").join("ChatGPT.exe"));
    }

    paths
}

fn codex_app_dir_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = vec![
        user_home.join("Applications").join("Codex.app"),
        user_home.join("Applications").join("ChatGPT.app"),
        PathBuf::from("/Applications/Codex.app"),
        PathBuf::from("/Applications/ChatGPT.app"),
    ];
    let packages = env_path("LOCALAPPDATA")
        .unwrap_or_else(|| user_home.join("AppData").join("Local"))
        .join("Packages");
    paths.push(packages.join("OpenAI.Codex_2p2nqsd0c76g0"));
    paths.push(packages.join("OpenAI.ChatGPT-Desktop_2p2nqsd0c76g0"));
    paths
}

fn windows_app_roots(user_home: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(local_app_data) = env_path("LOCALAPPDATA") {
        roots.push(local_app_data.join("Programs"));
        roots.push(local_app_data.join("Microsoft").join("WindowsApps"));
    } else {
        roots.push(user_home.join("AppData").join("Local").join("Programs"));
        roots.push(
            user_home
                .join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("WindowsApps"),
        );
    }
    for env_name in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env_path(env_name) {
            roots.push(root);
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::agents::codex::meta::{is_codex_app_installed_with, is_codex_cli_installed_with};
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

        assert!(is_codex_cli_installed_with("codex", &codex_home, &probe));
    }

    #[test]
    fn install_probes_separate_cli_and_desktop_app() {
        let dir = tempfile::tempdir().unwrap();
        let codex_home = dir.path().join(".codex");
        let app_home = dir.path().join("Applications").join("ChatGPT.app");
        std::fs::create_dir_all(&app_home).unwrap();
        let probe = InstallStatusProbe::test(command_never_exists, path_never_exists, path_is_dir);

        assert!(!is_codex_cli_installed_with("codex", &codex_home, &probe));
        assert!(is_codex_app_installed_with(&codex_home, &probe));
    }

    #[test]
    fn app_probe_accepts_store_package_family_directory() {
        let dir = tempfile::tempdir().unwrap();
        let codex_home = dir.path().join(".codex");
        let probe = InstallStatusProbe::test(
            command_never_exists,
            path_never_exists,
            only_codex_store_package_exists,
        );

        assert!(!is_codex_cli_installed_with("codex", &codex_home, &probe));
        assert!(is_codex_app_installed_with(&codex_home, &probe));
    }

    fn only_codex_command_exists(binary: &str) -> bool {
        binary == "codex"
    }

    fn command_never_exists(_: &str) -> bool {
        false
    }

    fn path_never_exists(_: &Path) -> bool {
        false
    }

    fn path_is_dir(path: &Path) -> bool {
        path.is_dir()
    }

    fn only_codex_store_package_exists(path: &Path) -> bool {
        path.ends_with("OpenAI.Codex_2p2nqsd0c76g0")
    }
}
