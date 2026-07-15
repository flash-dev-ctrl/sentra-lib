use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, PluginData, PluginInstallSource, PluginSourceKind};
use crate::utils::{dir_exists, is_directory, read_json_file};

#[derive(Debug, Clone)]
pub(super) struct PluginAsset {
    pub(crate) core: AssetCore,
    cache: Arc<Mutex<Option<Vec<PluginData>>>>,
}

impl PluginAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
            cache: Arc::new(Mutex::new(None)),
        }
    }
}

impl_erased_asset!(PluginAsset, AssetType::Plugin, Vec<PluginData>);

impl Asset<Vec<PluginData>> for PluginAsset {
    fn get_data(&self) -> SentraResult<Vec<PluginData>> {
        if let Some(cached) = self.cache.lock().unwrap().clone() {
            return Ok(cached);
        }
        let data = plugin_data(self.core.agent_home())?;
        *self.cache.lock().unwrap() = Some(data.clone());
        Ok(data)
    }
}

fn plugin_data(agent_home: &std::path::Path) -> SentraResult<Vec<PluginData>> {
    let cache_dir = agent_home.join("plugins").join("cache");
    let mut plugins = Vec::new();
    for manifest_path in find_plugin_manifests(&cache_dir, 8, 0) {
        let Some(manifest) = read_json_file(&manifest_path)? else {
            continue;
        };
        plugins.push(plugin_from_manifest(&cache_dir, &manifest_path, &manifest));
    }
    Ok(dedup_plugins(plugins))
}

fn plugin_from_manifest(
    cache_dir: &std::path::Path,
    manifest_path: &std::path::Path,
    manifest: &serde_json::Value,
) -> PluginData {
    let plugin_root = manifest_path
        .parent()
        .and_then(|path| path.parent())
        .map(std::path::Path::to_path_buf);
    let fallback_name = plugin_root
        .as_ref()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "plugin".to_string());
    let name = string_field(manifest, "name").unwrap_or(fallback_name);
    let version = string_field(manifest, "version");
    let marketplace = marketplace_segment(cache_dir, manifest_path);
    let reference = install_reference(&name, version.as_deref(), marketplace.as_deref())
        .unwrap_or_else(|| {
            plugin_root
                .as_ref()
                .unwrap_or(&manifest_path.to_path_buf())
                .display()
                .to_string()
        });
    let kind = if marketplace.is_some() {
        PluginSourceKind::Marketplace
    } else {
        PluginSourceKind::Cache
    };
    let interface = manifest.get("interface");

    PluginData {
        id: string_field(manifest, "id").or_else(|| Some(reference.clone())),
        name,
        display_name: interface
            .and_then(|value| string_field(value, "displayName"))
            .or_else(|| string_field(manifest, "displayName")),
        description: interface
            .and_then(|value| string_field(value, "description"))
            .or_else(|| string_field(manifest, "description")),
        version,
        author: author_name(manifest),
        enabled: manifest
            .get("enabled")
            .and_then(|value| value.as_bool())
            .or(Some(true)),
        origin: marketplace.clone().or_else(|| Some("cache".to_string())),
        install_source: Some(PluginInstallSource {
            kind,
            reference,
            marketplace,
        }),
        home: plugin_root,
        manifest_path: Some(manifest_path.to_path_buf()),
        capabilities: capabilities(manifest),
    }
}

fn find_plugin_manifests(
    dir: &std::path::Path,
    max_depth: usize,
    depth: usize,
) -> Vec<std::path::PathBuf> {
    if depth > max_depth || !dir_exists(dir) {
        return Vec::new();
    }
    let candidate = dir.join(".codex-plugin").join("plugin.json");
    if candidate.is_file() {
        return vec![candidate];
    }
    let mut results = Vec::new();
    for entry in read_dir_paths(dir) {
        if is_directory(&entry) {
            results.extend(find_plugin_manifests(&entry, max_depth, depth + 1));
        }
    }
    results
}

fn marketplace_segment(
    cache_dir: &std::path::Path,
    manifest_path: &std::path::Path,
) -> Option<String> {
    manifest_path
        .strip_prefix(cache_dir)
        .ok()
        .and_then(|path| path.components().next())
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
}

fn install_reference(
    name: &str,
    version: Option<&str>,
    marketplace: Option<&str>,
) -> Option<String> {
    let marketplace = marketplace?;
    Some(match version {
        Some(version) if !version.is_empty() => format!("{marketplace}/{name}@{version}"),
        _ => format!("{marketplace}/{name}"),
    })
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn author_name(plugin: &serde_json::Value) -> Option<String> {
    plugin.get("author").and_then(|author| {
        author.as_str().map(str::to_string).or_else(|| {
            author
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
    })
}

fn capabilities(plugin: &serde_json::Value) -> Vec<String> {
    let mut values = Vec::new();
    for key in [
        "skills",
        "commands",
        "agents",
        "hooks",
        "mcpServers",
        "tools",
    ] {
        if plugin.get(key).is_some_and(|value| !value.is_null()) {
            values.push(key.to_string());
        }
    }
    values
}

fn dedup_plugins(plugins: Vec<PluginData>) -> Vec<PluginData> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for plugin in plugins {
        let identity = plugin
            .install_source
            .as_ref()
            .map(|source| source.reference.clone())
            .or_else(|| plugin.id.clone())
            .unwrap_or_else(|| plugin.name.clone());
        let home = plugin.home.clone();
        if seen.insert((identity, home)) {
            result.push(plugin);
        }
    }
    result
}

fn read_dir_paths(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
}
