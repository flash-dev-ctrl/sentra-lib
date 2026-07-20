use crate::SentraResult;
use crate::agents::install_status::hidden_home_parent;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, PluginData, PluginInstallSource, PluginSourceKind};
use crate::utils::{dir_exists, read_json_file};

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
        let home = self.core.agent_home();
        let user_home = hidden_home_parent(home);
        let mut manifests = Vec::new();
        manifests.push(home.join(".cursor-plugin").join("plugin.json"));
        manifests.extend(find_manifests(
            &user_home.join(".cursor").join("plugins").join("local"),
            0,
        ));

        let mut plugins = Vec::new();
        for path in manifests {
            let Some(raw) = read_json_file(&path)? else {
                continue;
            };
            plugins.push(plugin_data(&path, &raw));
        }
        Ok(plugins)
    }
}

fn find_manifests(dir: &std::path::Path, depth: usize) -> Vec<std::path::PathBuf> {
    if depth > 6 || !dir_exists(dir) {
        return Vec::new();
    }
    let candidate = dir.join(".cursor-plugin").join("plugin.json");
    if candidate.is_file() {
        return vec![candidate];
    }
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .flat_map(|entry| find_manifests(&entry.path(), depth + 1))
        .collect()
}

fn plugin_data(path: &std::path::Path, raw: &serde_json::Value) -> PluginData {
    let name = string_field(raw, "name").unwrap_or_else(|| "cursor-plugin".to_string());
    PluginData {
        id: string_field(raw, "id").or_else(|| Some(name.clone())),
        name: name.clone(),
        display_name: string_field(raw, "displayName"),
        description: string_field(raw, "description"),
        version: string_field(raw, "version"),
        author: string_field(raw, "author"),
        enabled: raw
            .get("enabled")
            .and_then(|value| value.as_bool())
            .or(Some(true)),
        origin: Some("local".to_string()),
        install_source: Some(PluginInstallSource {
            kind: PluginSourceKind::LocalPath,
            reference: path.display().to_string(),
            marketplace: None,
        }),
        home: path
            .parent()
            .and_then(|path| path.parent())
            .map(std::path::Path::to_path_buf),
        manifest_path: Some(path.to_path_buf()),
        capabilities: ["skills", "agents", "hooks", "mcpServers"]
            .iter()
            .filter(|key| raw.get(**key).is_some())
            .map(|key| key.to_string())
            .collect(),
    }
}

fn string_field(raw: &serde_json::Value, key: &str) -> Option<String> {
    raw.get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
