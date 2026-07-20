use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, PluginData, PluginInstallSource, PluginSourceKind};
use crate::utils::{dir_exists, is_directory, read_json_file};

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
        plugin_data(self.core.agent_home())
    }
}

fn plugin_data(agent_home: &std::path::Path) -> SentraResult<Vec<PluginData>> {
    let dir = agent_home.join("plugins");
    if !dir_exists(&dir) {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for plugin_dir in std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
    {
        if !is_directory(&plugin_dir) {
            continue;
        }
        for manifest_path in [
            plugin_dir.join(".codebuddy-plugin").join("plugin.json"),
            plugin_dir.join("plugin.json"),
        ] {
            let Some(manifest) = read_json_file(&manifest_path)? else {
                continue;
            };
            out.push(plugin(&manifest, &manifest_path, &plugin_dir));
            break;
        }
    }
    Ok(out)
}

fn plugin(
    manifest: &serde_json::Value,
    manifest_path: &std::path::Path,
    home: &std::path::Path,
) -> PluginData {
    let name = string(manifest, "name")
        .or_else(|| {
            home.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "plugin".to_string());
    PluginData {
        id: string(manifest, "id").or_else(|| Some(name.clone())),
        name: name.clone(),
        display_name: string(manifest, "displayName"),
        description: string(manifest, "description"),
        version: string(manifest, "version"),
        author: string(manifest, "author"),
        enabled: manifest
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .or(Some(true)),
        origin: Some("plugins".to_string()),
        install_source: Some(PluginInstallSource {
            kind: PluginSourceKind::LocalPath,
            reference: home.display().to_string(),
            marketplace: None,
        }),
        home: Some(home.to_path_buf()),
        manifest_path: Some(manifest_path.to_path_buf()),
        capabilities: ["skills", "commands", "mcpServers"]
            .into_iter()
            .filter(|key| manifest.get(*key).is_some())
            .map(str::to_string)
            .collect(),
    }
}

fn string(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_string)
}
