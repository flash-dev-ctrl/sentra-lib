use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_existing_dir_with, any_existing_file_with, binary_paths, env_path,
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
        name: agent_name.to_string(),
        description: Some(
            "Anthropic's native desktop application with MCP server support.".to_string(),
        ),
        version: None,
        author: Some("Anthropic".to_string()),
        installed,
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
}

pub(super) fn is_agent_installed(_agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real(claude_app_user_home(agent_home));
    is_agent_installed_with(agent_home, &probe)
}

fn is_agent_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_existing_file_with(claude_app_install_paths(agent_home), probe)
        || any_existing_dir_with(claude_app_dir_paths(agent_home), probe)
        || probe.product_installed(&["Claude"], &["Anthropic"])
}

fn claude_app_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = claude_app_user_home(agent_home);
    let mut paths = binary_paths(agent_home, "Claude");
    paths.extend(binary_paths(agent_home.join("app"), "Claude"));
    for local_app_data in local_app_data_roots(&user_home) {
        paths.extend(binary_paths(
            local_app_data.join("Programs").join("Claude"),
            "Claude",
        ));
    }
    paths
}

fn claude_app_dir_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = claude_app_user_home(agent_home);
    let mut paths = vec![
        user_home.join("Applications").join("Claude.app"),
        PathBuf::from("/Applications/Claude.app"),
    ];
    paths.extend(
        local_app_data_roots(&user_home)
            .into_iter()
            .map(|root| root.join("Packages").join("Claude_pzs8sxrjxfjjc")),
    );
    paths
}

fn local_app_data_roots(user_home: &Path) -> Vec<PathBuf> {
    let default = user_home.join("AppData").join("Local");
    let mut roots = vec![default.clone()];
    if let Some(root) = env_path("LOCALAPPDATA")
        && root != default
    {
        roots.push(root);
    }
    roots
}

fn claude_app_user_home(agent_home: &Path) -> PathBuf {
    let parts = agent_home
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for suffix in [
        &["AppData", "Local", "Claude"][..],
        &["AppData", "Local", "Claude-3p"][..],
        &["Library", "Application Support", "Claude"][..],
        &["Library", "Application Support", "Claude-3p"][..],
    ] {
        if path_parts_end_with(&parts, suffix) {
            let ancestor_count = suffix.len();
            let mut home = agent_home;
            for _ in 0..ancestor_count {
                home = home.parent().unwrap_or(home);
            }
            return home.to_path_buf();
        }
    }
    hidden_home_parent(agent_home)
}

fn path_parts_end_with(parts: &[String], suffix: &[&str]) -> bool {
    parts.len() >= suffix.len()
        && parts[parts.len() - suffix.len()..]
            .iter()
            .map(String::as_str)
            .eq(suffix.iter().copied())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::agents::claude::app_meta::is_agent_installed_with;
    use crate::agents::install_status::InstallStatusProbe;

    #[test]
    fn install_probe_requires_app_binary_or_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let app_home = dir.path().join("AppData").join("Local").join("Claude");
        std::fs::create_dir_all(&app_home).unwrap();
        let probe =
            InstallStatusProbe::test(command_never_exists, path_never_exists, path_never_exists);

        assert!(!is_agent_installed_with(&app_home, &probe));

        let app_dir = dir
            .path()
            .join("AppData")
            .join("Local")
            .join("Programs")
            .join("Claude");
        std::fs::create_dir_all(&app_dir).unwrap();
        let app_binary = app_dir.join(if cfg!(windows) {
            "Claude.exe"
        } else {
            "Claude"
        });
        std::fs::write(&app_binary, "").unwrap();
        let probe = InstallStatusProbe::test(command_never_exists, path_is_file, path_never_exists);

        assert!(is_agent_installed_with(&app_home, &probe));
    }

    #[test]
    fn install_probe_accepts_store_package_family() {
        let dir = tempfile::tempdir().unwrap();
        let app_home = dir.path().join("AppData").join("Local").join("Claude");
        let probe = InstallStatusProbe::test(
            command_never_exists,
            path_never_exists,
            only_claude_package_exists,
        );

        assert!(is_agent_installed_with(&app_home, &probe));
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

    fn only_claude_package_exists(path: &Path) -> bool {
        path.ends_with("Claude_pzs8sxrjxfjjc")
    }
}
