use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::agents::trae::surface;
use crate::interfaces::{
    Asset, AssetType, MetaData, PluginData, PluginInstallSource, PluginSourceKind,
};
use crate::utils::{dir_exists, read_json_file};

#[derive(Debug, Clone)]
pub(super) struct MetaAsset {
    core: AssetCore,
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
        let installed = is_agent_installed(home);
        if !dir_exists(home) && !installed {
            return Ok(None);
        }
        Ok(Some(MetaData {
            id: Some(self.core.agent_name().to_string()),
            name: surface::title(self.core.agent_name()).to_string(),
            description: Some(
                "TRAE VS Code plugin installed in VS Code extension caches.".to_string(),
            ),
            version: plugin_version(home)?,
            author: Some("ByteDance".to_string()),
            installed,
            home: Some(home.to_path_buf()),
            ..MetaData::default()
        }))
    }
}

#[derive(Debug, Clone)]
pub(super) struct PluginAsset {
    core: AssetCore,
}

impl PluginAsset {
    pub(super) fn new(agent_name: impl Into<String>, agent_home: impl Into<PathBuf>) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(PluginAsset, AssetType::Plugin, Vec<PluginData>);

impl Asset<Vec<PluginData>> for PluginAsset {
    fn get_data(&self) -> SentraResult<Vec<PluginData>> {
        plugin_data(self.core.agent_home())
    }
}

pub(super) fn is_agent_installed(agent_home: &Path) -> bool {
    vscode_plugin_dirs(agent_home).into_iter().any(|dir| {
        read_json_file(dir.join("package.json"))
            .ok()
            .flatten()
            .is_some_and(|manifest| is_trae_vscode_manifest(&manifest))
    })
}

fn plugin_version(agent_home: &Path) -> SentraResult<Option<String>> {
    for dir in vscode_plugin_dirs(agent_home) {
        let Some(manifest) = read_json_file(dir.join("package.json"))? else {
            continue;
        };
        if is_trae_vscode_manifest(&manifest) {
            return Ok(string_field(&manifest, "version"));
        }
    }
    Ok(None)
}

fn plugin_data(agent_home: &Path) -> SentraResult<Vec<PluginData>> {
    let mut seen = HashSet::new();
    let mut plugins = Vec::new();
    for dir in vscode_plugin_dirs(agent_home) {
        let manifest_path = dir.join("package.json");
        let Some(manifest) = read_json_file(&manifest_path)? else {
            continue;
        };
        if !is_trae_vscode_manifest(&manifest) {
            continue;
        }
        let Some(plugin) = plugin_from_manifest(&manifest, &manifest_path, &dir) else {
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

fn vscode_plugin_dirs(agent_home: &Path) -> Vec<PathBuf> {
    let vscode_home = surface::vscode_extensions_home(agent_home);
    let mut dirs = extension_dirs(vscode_home.join("extensions"));
    if let Some(root) = crate::agents::install_status::env_path("VSCODE_EXTENSIONS") {
        dirs.extend(extension_dirs(root));
    }
    dirs
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

fn plugin_from_manifest(
    manifest: &serde_json::Value,
    manifest_path: &Path,
    home: &Path,
) -> Option<PluginData> {
    let name = string_field(manifest, "name").or_else(|| {
        home.file_name()
            .map(|name| name.to_string_lossy().to_string())
    })?;
    let publisher = string_field(manifest, "publisher");
    let id = extension_id(publisher.as_deref(), &name);
    Some(PluginData {
        id: Some(id.clone()),
        name,
        display_name: string_field(manifest, "displayName"),
        description: string_field(manifest, "description"),
        version: string_field(manifest, "version"),
        author: publisher,
        enabled: Some(true),
        origin: Some("vscode-extension".to_string()),
        install_source: Some(PluginInstallSource {
            kind: PluginSourceKind::Cache,
            reference: id,
            marketplace: Some("vscode".to_string()),
        }),
        home: Some(home.to_path_buf()),
        manifest_path: Some(manifest_path.to_path_buf()),
        capabilities: capabilities(manifest),
    })
}

fn is_trae_vscode_manifest(manifest: &serde_json::Value) -> bool {
    let name = string_field(manifest, "name").unwrap_or_default();
    let publisher = string_field(manifest, "publisher").unwrap_or_default();
    let id = extension_id(Some(&publisher), &name).to_ascii_lowercase();
    if surface::TRAE_VSCODE_EXTENSION_IDS
        .iter()
        .any(|expected| id == *expected)
    {
        return true;
    }
    publisher.eq_ignore_ascii_case("MarsCode")
        && string_field(manifest, "displayName")
            .is_some_and(|display_name| display_name.to_ascii_lowercase().contains("trae"))
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
    fn vscode_plugin_surface_keeps_only_trae_plugin_from_vscode_home() {
        let dir = tempfile::tempdir().unwrap();
        let vscode = dir.path().join(".vscode");
        let extensions = vscode.join("extensions");
        let trae = extensions.join("marscode.marscode-extension-1.7.3");
        let other = extensions.join("ms-python.python-1.0.0");
        std::fs::create_dir_all(&trae).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(
            trae.join("package.json"),
            r#"{"name":"marscode-extension","publisher":"MarsCode","displayName":"TRAE AI: Coding Assistant","version":"1.7.3"}"#,
        )
        .unwrap();
        std::fs::write(
            other.join("package.json"),
            r#"{"name":"python","publisher":"ms-python","displayName":"Python"}"#,
        )
        .unwrap();

        let plugins = <PluginAsset as Asset<Vec<PluginData>>>::get_data(&PluginAsset::new(
            "trae-vscode-plugin",
            &vscode,
        ))
        .unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(
            plugins[0].id.as_deref(),
            Some("MarsCode.marscode-extension")
        );
        assert!(is_agent_installed(&vscode));
        assert_eq!(plugin_version(&vscode).unwrap().as_deref(), Some("1.7.3"));
    }
}
