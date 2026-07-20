use std::collections::HashSet;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, PluginData, PluginInstallSource, PluginSourceKind};
use crate::utils::read_json_file;

#[derive(Debug, Clone)]
pub(super) struct PluginAsset {
    pub(crate) core: AssetCore,
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
        let mut seen = HashSet::new();
        let mut plugins = Vec::new();
        for extension_dir in super::extension_dirs(self.core.agent_home()) {
            let manifest_path = extension_dir.join("package.json");
            let Some(manifest) = read_json_file(&manifest_path)? else {
                continue;
            };
            if !super::is_agent_manifest(&manifest) {
                continue;
            }
            let name = string_field(&manifest, "name").unwrap_or_else(|| "extension".to_string());
            let publisher = string_field(&manifest, "publisher");
            let id = publisher
                .as_ref()
                .map(|publisher| format!("{publisher}.{name}"))
                .unwrap_or_else(|| name.clone());
            if seen.insert(id.clone()) {
                plugins.push(PluginData {
                    id: Some(id.clone()),
                    name,
                    display_name: string_field(&manifest, "displayName"),
                    description: string_field(&manifest, "description"),
                    version: string_field(&manifest, "version"),
                    author: publisher,
                    enabled: Some(true),
                    origin: Some("vscode-extension".to_string()),
                    install_source: Some(PluginInstallSource {
                        kind: PluginSourceKind::Cache,
                        reference: id,
                        marketplace: Some("vscode".to_string()),
                    }),
                    home: Some(extension_dir),
                    manifest_path: Some(manifest_path),
                    capabilities: capabilities(&manifest),
                });
            }
        }
        for manifest_path in super::agent_plugin_manifests(self.core.agent_home()) {
            let Some(manifest) = read_json_file(&manifest_path)? else {
                continue;
            };
            if !super::is_agent_manifest(&manifest) {
                continue;
            }
            let name =
                string_field(&manifest, "name").unwrap_or_else(|| "agent-plugin".to_string());
            let id = string_field(&manifest, "id").unwrap_or_else(|| name.clone());
            if seen.insert(id.clone()) {
                plugins.push(PluginData {
                    id: Some(id.clone()),
                    name,
                    display_name: string_field(&manifest, "displayName"),
                    description: string_field(&manifest, "description"),
                    version: string_field(&manifest, "version"),
                    author: string_field(&manifest, "publisher")
                        .or_else(|| string_field(&manifest, "author")),
                    enabled: Some(true),
                    origin: Some("agentPlugins-cache".to_string()),
                    install_source: manifest_path.parent().map(|home| PluginInstallSource {
                        kind: PluginSourceKind::Cache,
                        reference: home.display().to_string(),
                        marketplace: Some("vscode".to_string()),
                    }),
                    home: manifest_path.parent().map(std::path::Path::to_path_buf),
                    manifest_path: Some(manifest_path),
                    capabilities: capabilities(&manifest),
                });
            }
        }
        Ok(plugins)
    }
}

pub(super) fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn capabilities(manifest: &serde_json::Value) -> Vec<String> {
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
