use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_dir_with, any_existing_file_with,
    binary_paths, hidden_home_parent, is_ide_extension_installed, user_home_for_agent_home,
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
        name: crate::agents::discovery::get_agent_title(agent_name),
        description: Some(description(agent_name).to_string()),
        version: None,
        author: Some("Moonshot AI".to_string()),
        installed,
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
}

fn description(agent_name: &str) -> &'static str {
    if agent_name == crate::agents::entries::KIMI_APP_AGENT_ENTRY.name {
        "Kimi desktop application with Daimon skills, plugins, providers, memory, and Automations."
    } else if agent_name == crate::agents::entries::KIMI_CLI_IDE_AGENT_ENTRY.name {
        "Kimi Code IDE extension sharing KIMI_CODE_HOME configuration and sessions with the CLI."
    } else {
        "Kimi Code CLI user configuration, providers, MCP servers, skills, plugins, and cron tasks."
    }
}

pub(super) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    if agent_name == crate::agents::entries::KIMI_CLI_IDE_AGENT_ENTRY.name {
        return is_ide_extension_installed(
            agent_home,
            crate::agents::kimi::KIMI_CODE_IDE_EXTENSION_ID,
        );
    }
    if agent_name == crate::agents::entries::KIMI_APP_AGENT_ENTRY.name {
        let probe = InstallStatusProbe::real(kimi_app_user_home(agent_home));
        is_kimi_app_installed_with(agent_home, &probe)
    } else {
        let probe = InstallStatusProbe::real(user_home_for_agent_home(agent_home, &[".kimi-code"]));
        is_kimi_cli_installed_with(agent_home, &probe)
    }
}

fn is_kimi_cli_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_command_exists_with(&["kimi"], probe)
        || any_existing_file_with(kimi_install_paths(agent_home), probe)
}

fn is_kimi_app_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    if !app_home_matches_current_platform(agent_home) {
        return false;
    }
    any_existing_file_with(kimi_app_install_paths(agent_home), probe)
        || any_existing_dir_with(kimi_app_bundle_paths(agent_home), probe)
}

fn kimi_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "kimi");
    paths.extend(binary_paths(agent_home.join("bin"), "kimi"));
    paths.extend(binary_paths(
        user_home.join("AppData").join("Roaming").join("npm"),
        "kimi",
    ));
    paths
}

fn kimi_app_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = kimi_app_user_home(agent_home);
    binary_paths(
        user_home
            .join("AppData")
            .join("Local")
            .join("Programs")
            .join("kimi-desktop"),
        "Kimi",
    )
}

fn kimi_app_bundle_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = kimi_app_user_home(agent_home);
    vec![
        user_home.join("Applications").join("Kimi.app"),
        PathBuf::from("/Applications/Kimi.app"),
    ]
}

fn app_home_matches_current_platform(agent_home: &Path) -> bool {
    if cfg!(windows) {
        path_ends_with(agent_home, &["AppData", "Roaming", "kimi-desktop"])
    } else if cfg!(target_os = "macos") {
        path_ends_with(
            agent_home,
            &["Library", "Application Support", "kimi-desktop"],
        )
    } else {
        path_ends_with(agent_home, &[".config", "kimi-desktop"])
    }
}

fn kimi_app_user_home(agent_home: &Path) -> PathBuf {
    for suffix in [
        &["AppData", "Roaming", "kimi-desktop"][..],
        &["Library", "Application Support", "kimi-desktop"][..],
        &[".config", "kimi-desktop"][..],
    ] {
        if path_ends_with(agent_home, suffix) {
            let mut home = agent_home;
            for _ in suffix {
                home = home.parent().unwrap_or(home);
            }
            return home.to_path_buf();
        }
    }
    hidden_home_parent(agent_home)
}

fn path_ends_with(path: &Path, suffix: &[&str]) -> bool {
    let parts = path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>();
    parts.len() >= suffix.len()
        && parts[parts.len() - suffix.len()..]
            .iter()
            .zip(suffix)
            .all(|(actual, expected)| actual.eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::agents::install_status::InstallStatusProbe;
    use crate::agents::kimi::meta::{
        is_kimi_app_installed_with, is_kimi_cli_installed_with, kimi_app_user_home,
    };

    #[test]
    fn install_probes_separate_cli_and_app() {
        let dir = tempfile::tempdir().unwrap();
        let cli_home = dir.path().join(".kimi-code");
        let app_home = dir
            .path()
            .join("AppData")
            .join("Roaming")
            .join("kimi-desktop");
        let cli_probe = InstallStatusProbe::test(|binary| binary == "kimi", |_| false, |_| false);
        let app_probe = InstallStatusProbe::test(
            |_| false,
            |path| path.ends_with(Path::new("Programs/kimi-desktop/Kimi.exe")),
            |_| false,
        );

        assert!(is_kimi_cli_installed_with(&cli_home, &cli_probe));
        assert!(!is_kimi_cli_installed_with(&cli_home, &app_probe));
        if cfg!(windows) {
            assert!(is_kimi_app_installed_with(&app_home, &app_probe));
            assert!(!is_kimi_app_installed_with(&app_home, &cli_probe));
        }
    }

    #[test]
    fn app_home_resolves_user_home() {
        let home = Path::new("Users").join("me");
        let app_home = home.join("AppData").join("Roaming").join("kimi-desktop");

        assert_eq!(kimi_app_user_home(&app_home), home);
    }
}
