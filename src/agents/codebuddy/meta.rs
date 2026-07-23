use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::codebuddy::surface;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_dir_with, any_existing_file_with,
    binary_paths, env_path, is_ide_extension_installed, is_named_cli_agent_installed_with,
    user_home_for_agent_home,
};
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, MetaData};
use crate::utils::dir_exists;

#[derive(Debug, Clone)]
pub(super) struct MetaAsset {
    core: AssetCore,
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

fn meta_data(agent_name: &str, agent_home: &Path) -> SentraResult<Option<MetaData>> {
    let installed = is_agent_installed(agent_name, agent_home);
    if !dir_exists(agent_home) && !installed {
        return Ok(None);
    }
    Ok(Some(MetaData {
        id: Some(agent_name.to_string()),
        name: surface::title(agent_name).to_string(),
        description: Some(description(agent_name).to_string()),
        version: None,
        author: Some("Tencent Cloud".to_string()),
        installed,
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
}

fn description(agent_name: &str) -> &'static str {
    if surface::is_ide(agent_name) {
        "CodeBuddy desktop AI code editor configuration and automation state."
    } else if surface::is_ide_extension(agent_name) {
        "CodeBuddy IDE extension installed in VS Code-compatible extension indexes."
    } else {
        "CodeBuddy CLI user configuration, providers, MCP servers, skills, plugins, and memory."
    }
}

pub(super) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    if surface::is_ide_extension(agent_name) {
        return is_ide_extension_installed(agent_home, surface::CODEBUDDY_IDE_EXTENSION_ID);
    }
    if surface::is_ide(agent_name) {
        let probe = InstallStatusProbe::real(surface::ide_user_home(agent_home));
        return is_codebuddy_ide_installed_with(agent_name, agent_home, &probe);
    }
    if !surface::is_cli(agent_name) {
        return false;
    }
    let probe = InstallStatusProbe::real(user_home_for_agent_home(agent_home, &[".codebuddy"]));
    is_named_cli_agent_installed_with("codebuddy", agent_home, &probe)
        || is_named_cli_agent_installed_with("codebuddy-cli", agent_home, &probe)
}

fn is_codebuddy_ide_installed_with(
    agent_name: &str,
    agent_home: &Path,
    probe: &InstallStatusProbe,
) -> bool {
    if !ide_home_matches_current_platform(agent_name, agent_home) {
        return false;
    }
    let products: &[&str] = if surface::is_cn(agent_name) {
        &["CodeBuddy CN"]
    } else {
        &["CodeBuddy"]
    };
    any_command_exists_with(ide_commands(agent_name), probe)
        || any_existing_file_with(ide_install_paths(agent_name, agent_home), probe)
        || any_existing_dir_with(ide_app_paths(agent_name, agent_home), probe)
        || probe.product_installed(products, &["Tencent", "腾讯"])
}

fn ide_commands(agent_name: &str) -> &'static [&'static str] {
    if surface::is_cn(agent_name) {
        &["CodeBuddy CN", "buddycn"]
    } else {
        &["CodeBuddy", "codebuddy-ide"]
    }
}

fn ide_install_paths(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let user_home = surface::ide_user_home(agent_home);
    let app_name = surface::ide_app_name(agent_name);
    let mut paths = Vec::new();
    let local_app_data =
        env_path("LOCALAPPDATA").unwrap_or_else(|| user_home.join("AppData").join("Local"));
    paths.extend(binary_paths(
        local_app_data.join("Programs").join(app_name),
        app_name,
    ));
    if surface::is_cn(agent_name) {
        paths.extend(binary_paths(
            local_app_data.join("Programs").join(app_name).join("bin"),
            "buddycn",
        ));
    }
    paths
}

fn ide_app_paths(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let user_home = surface::ide_user_home(agent_home);
    let app_name = format!("{}.app", surface::ide_app_name(agent_name));
    vec![
        user_home.join("Applications").join(&app_name),
        PathBuf::from("/Applications").join(app_name),
    ]
}

fn ide_home_matches_current_platform(agent_name: &str, agent_home: &Path) -> bool {
    let app_name = surface::ide_app_name(agent_name);
    if cfg!(windows) {
        surface::path_ends_with(agent_home, &["AppData", "Roaming", app_name])
    } else if cfg!(target_os = "macos") {
        surface::path_ends_with(agent_home, &["Library", "Application Support", app_name])
    } else {
        surface::path_ends_with(agent_home, &[".config", app_name])
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::agents::codebuddy::meta::is_codebuddy_ide_installed_with;
    use crate::agents::install_status::InstallStatusProbe;

    #[test]
    fn ide_probe_accepts_cn_desktop_install_on_matching_platform() {
        let dir = tempfile::tempdir().unwrap();
        let app_home = dir
            .path()
            .join("AppData")
            .join("Roaming")
            .join("CodeBuddy CN");
        let probe = InstallStatusProbe::test(
            |_| false,
            |path| path.ends_with(Path::new("Programs/CodeBuddy CN/CodeBuddy CN.exe")),
            |_| false,
        );

        if cfg!(windows) {
            assert!(is_codebuddy_ide_installed_with(
                "codebuddy-cn-ide",
                &app_home,
                &probe
            ));
        }
    }
}
