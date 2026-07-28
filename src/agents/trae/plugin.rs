use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::agents::trae::surface;
use crate::interfaces::{Asset, AssetType, PluginData, PluginInstallSource, PluginSourceKind};
use crate::utils::read_json_file;

#[derive(Debug, Clone)]
pub(super) struct PluginAsset {
    core: AssetCore,
}

impl PluginAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(PluginAsset, AssetType::Plugin, Vec<PluginData>);

impl Asset<Vec<PluginData>> for PluginAsset {
    fn get_data(&self) -> SentraResult<Vec<PluginData>> {
        ide_extension_data(self.core.agent_name(), self.core.agent_home())
    }
}

fn ide_extension_data(agent_name: &str, agent_home: &Path) -> SentraResult<Vec<PluginData>> {
    let state_home = surface::state_home(agent_name, agent_home);
    let mut dirs = Vec::new();
    dirs.extend(extension_dirs(state_home.join("extensions")));
    dirs.extend(extension_dirs(agent_home.join("User").join("extensions")));
    dirs.extend(extension_dirs(agent_home.join("extensions")));
    for app_root in surface::ide_data_roots(agent_name, agent_home) {
        dirs.extend(extension_dirs(app_root.join("User").join("extensions")));
        dirs.extend(extension_dirs(app_root.join("extensions")));
    }
    collect_plugins(dirs, PluginOrigin::TraeIde)
}

fn extension_dirs(root: PathBuf) -> Vec<PathBuf> {
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect()
}

fn collect_plugins(dirs: Vec<PathBuf>, origin: PluginOrigin) -> SentraResult<Vec<PluginData>> {
    let mut seen = HashSet::new();
    let mut plugins = Vec::new();
    for dir in dirs {
        let manifest_path = dir.join("package.json");
        let Some(manifest) = read_json_file(&manifest_path)? else {
            continue;
        };
        let Some(plugin) = plugin_from_manifest(&manifest, &manifest_path, &dir, origin) else {
            continue;
        };
        let identity = plugin
            .id
            .clone()
            .unwrap_or_else(|| plugin.name.clone())
            .to_ascii_lowercase();
        if seen.insert(identity) {
            plugins.push(plugin);
        }
    }
    Ok(plugins)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginOrigin {
    TraeIde,
}

fn plugin_from_manifest(
    manifest: &serde_json::Value,
    manifest_path: &Path,
    home: &Path,
    origin: PluginOrigin,
) -> Option<PluginData> {
    let name = string_field(manifest, "name").or_else(|| {
        home.file_name()
            .map(|name| name.to_string_lossy().to_string())
    })?;
    let publisher = string_field(manifest, "publisher");
    let id = extension_id(publisher.as_deref(), &name);
    let (origin_name, marketplace) = match origin {
        PluginOrigin::TraeIde => ("trae-extension", "trae"),
    };
    Some(PluginData {
        id: Some(id.clone()),
        name,
        display_name: string_field(manifest, "displayName"),
        description: string_field(manifest, "description"),
        version: string_field(manifest, "version"),
        author: publisher,
        enabled: Some(true),
        origin: Some(origin_name.to_string()),
        install_source: Some(PluginInstallSource {
            kind: PluginSourceKind::Cache,
            reference: id,
            marketplace: Some(marketplace.to_string()),
        }),
        home: Some(home.to_path_buf()),
        manifest_path: Some(manifest_path.to_path_buf()),
        capabilities: capabilities(manifest),
    })
}

fn extension_id(publisher: Option<&str>, name: &str) -> String {
    publisher
        .filter(|publisher| !publisher.trim().is_empty())
        .map(|publisher| format!("{publisher}.{name}"))
        .unwrap_or_else(|| name.to_string())
}

fn capabilities(manifest: &serde_json::Value) -> Vec<String> {
    let Some(contributes) = manifest
        .get("contributes")
        .and_then(|value| value.as_object())
    else {
        return Vec::new();
    };
    contributes
        .keys()
        .filter(|key| {
            [
                "commands",
                "views",
                "chatParticipants",
                "chatParticipant",
                "chatAgents",
                "chatSkills",
                "skills",
                "agentPlugins",
                "agentTools",
                "languageModelTools",
                "mcpServerDefinitionProviders",
            ]
            .contains(&key.as_str())
        })
        .cloned()
        .collect()
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use crate::interfaces::{Asset, PluginData};

    use super::*;

    #[test]
    fn reads_trae_ide_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".trae");
        let extension = home
            .join("extensions")
            .join("marscode.marscode-extension-1.7.3");
        std::fs::create_dir_all(&extension).unwrap();
        std::fs::write(
            extension.join("package.json"),
            r#"{"name":"marscode-extension","publisher":"MarsCode","displayName":"TRAE AI: Coding Assistant","version":"1.7.3","contributes":{"commands":[]}}"#,
        )
        .unwrap();

        let plugins =
            <PluginAsset as Asset<Vec<PluginData>>>::get_data(&PluginAsset::new("trae-ide", &home))
                .unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(
            plugins[0].id.as_deref(),
            Some("MarsCode.marscode-extension")
        );
        assert_eq!(plugins[0].origin.as_deref(), Some("trae-extension"));
        assert_eq!(plugins[0].capabilities, ["commands"]);
    }
}
