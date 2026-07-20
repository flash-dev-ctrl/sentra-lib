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
        let root = super::user_home(self.core.agent_home())
            .join(".vscode")
            .join("extensions");
        let mut plugins = Vec::new();
        for dir in super::read_dir_paths(&root) {
            let manifest_path = dir.join("package.json");
            let Some(manifest) = read_json_file(&manifest_path)? else {
                continue;
            };
            let id = extension_id(&manifest);
            if id.as_deref() != Some("coder.coder-remote") {
                continue;
            }
            plugins.push(PluginData {
                id: id.clone(),
                name: string_field(&manifest, "name").unwrap_or_else(|| "coder-remote".to_string()),
                display_name: string_field(&manifest, "displayName"),
                description: string_field(&manifest, "description"),
                version: string_field(&manifest, "version"),
                author: string_field(&manifest, "publisher"),
                enabled: Some(true),
                origin: Some("vscode-extension".to_string()),
                install_source: id.map(|id| PluginInstallSource {
                    kind: PluginSourceKind::Cache,
                    reference: id,
                    marketplace: Some("vscode".to_string()),
                }),
                home: Some(dir),
                manifest_path: Some(manifest_path),
                capabilities: vec!["remote".to_string()],
            });
        }
        Ok(plugins)
    }
}

fn extension_id(value: &serde_json::Value) -> Option<String> {
    Some(format!(
        "{}.{}",
        string_field(value, "publisher")?,
        string_field(value, "name")?
    ))
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
