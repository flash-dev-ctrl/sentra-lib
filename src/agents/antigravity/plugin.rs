use std::collections::HashSet;

use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{Asset, AssetType, PluginData, PluginInstallSource, PluginSourceKind};
use crate::utils::read_json_file;
use crate::SentraResult;

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
        for manifest_path in super::plugin_manifests(self.core.agent_home()) {
            let Some(manifest) = read_json_file(&manifest_path)? else {
                continue;
            };
            let name = string_field(&manifest, "name").unwrap_or_else(|| "plugin".to_string());
            let home = manifest_path.parent().map(std::path::Path::to_path_buf);
            if seen.insert((name.clone(), home.clone())) {
                plugins.push(PluginData {
                    id: string_field(&manifest, "id").or_else(|| Some(name.clone())),
                    name: name.clone(),
                    display_name: string_field(&manifest, "displayName"),
                    description: string_field(&manifest, "description"),
                    version: string_field(&manifest, "version"),
                    author: author_name(&manifest),
                    enabled: manifest
                        .get("enabled")
                        .and_then(|value| value.as_bool())
                        .or(Some(true)),
                    origin: Some("antigravity".to_string()),
                    install_source: home.as_ref().map(|home| PluginInstallSource {
                        kind: PluginSourceKind::LocalPath,
                        reference: home.display().to_string(),
                        marketplace: None,
                    }),
                    home,
                    manifest_path: Some(manifest_path),
                    capabilities: capabilities(&manifest),
                });
            }
        }
        Ok(plugins)
    }
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn author_name(value: &serde_json::Value) -> Option<String> {
    value.get("author").and_then(|author| {
        author.as_str().map(str::to_string).or_else(|| {
            author
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
    })
}

fn capabilities(value: &serde_json::Value) -> Vec<String> {
    [
        "skills",
        "mcpServers",
        "commands",
        "tools",
        "scheduled",
        "cron",
        "tasks",
    ]
    .into_iter()
    .filter(|key| value.get(*key).is_some_and(|value| !value.is_null()))
    .map(str::to_string)
    .collect()
}
