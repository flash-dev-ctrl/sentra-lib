use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, PluginData, PluginInstallSource, PluginSourceKind};
use crate::utils::{dir_exists, is_directory, read_json_file};

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
        plugin_data(self.core.agent_home())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct KimiPluginManifest {
    pub(crate) root: std::path::PathBuf,
    pub(crate) path: std::path::PathBuf,
    pub(crate) value: Value,
    pub(crate) enabled: bool,
}

pub(crate) fn plugin_manifests(
    agent_home: &std::path::Path,
) -> SentraResult<Vec<KimiPluginManifest>> {
    let enabled = installed_enabled_map(agent_home)?;
    let mut manifests = Vec::new();
    for path in find_plugin_manifests(&agent_home.join("plugins").join("managed"), 8, 0) {
        let Some(value) = read_json_file(&path)? else {
            continue;
        };
        let root = plugin_root(&path).unwrap_or_else(|| {
            path.parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(|| path.clone())
        });
        let manifest_enabled = manifest_enabled(&value, &root, &enabled);
        manifests.push(KimiPluginManifest {
            root,
            path,
            value,
            enabled: manifest_enabled,
        });
    }
    Ok(manifests)
}

fn plugin_data(agent_home: &std::path::Path) -> SentraResult<Vec<PluginData>> {
    let plugins = plugin_manifests(agent_home)?
        .into_iter()
        .map(plugin_from_manifest)
        .collect();
    Ok(dedup_plugins(plugins))
}

fn plugin_from_manifest(item: KimiPluginManifest) -> PluginData {
    let fallback_name = item
        .root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "plugin".to_string());
    let name = string_field(&item.value, "name").unwrap_or(fallback_name);
    let interface = item.value.get("interface");
    let reference = item.root.display().to_string();

    PluginData {
        id: string_field(&item.value, "id").or_else(|| Some(name.clone())),
        name,
        display_name: interface
            .and_then(|value| string_field(value, "displayName"))
            .or_else(|| string_field(&item.value, "displayName"))
            .or_else(|| interface.and_then(|value| string_field(value, "shortDescription"))),
        description: interface
            .and_then(|value| string_field(value, "description"))
            .or_else(|| interface.and_then(|value| string_field(value, "shortDescription")))
            .or_else(|| string_field(&item.value, "description")),
        version: string_field(&item.value, "version"),
        author: author_name(&item.value),
        enabled: Some(item.enabled),
        origin: Some("managed".to_string()),
        install_source: Some(PluginInstallSource {
            kind: PluginSourceKind::LocalPath,
            reference,
            marketplace: None,
        }),
        home: Some(item.root),
        manifest_path: Some(item.path),
        capabilities: capabilities(&item.value),
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
    let primary = dir.join("kimi.plugin.json");
    if primary.is_file() {
        return vec![primary];
    }
    let secondary = dir.join(".kimi-plugin").join("plugin.json");
    if secondary.is_file() {
        return vec![secondary];
    }
    let mut results = Vec::new();
    for entry in read_dir_paths(dir) {
        if is_directory(&entry) {
            results.extend(find_plugin_manifests(&entry, max_depth, depth + 1));
        }
    }
    results
}

fn plugin_root(path: &std::path::Path) -> Option<std::path::PathBuf> {
    if path.file_name().and_then(|name| name.to_str()) == Some("kimi.plugin.json") {
        return path.parent().map(std::path::Path::to_path_buf);
    }
    path.parent()
        .and_then(|path| path.parent())
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some(".kimi-plugin"))
        .and_then(|path| path.parent())
        .map(std::path::Path::to_path_buf)
        .or_else(|| {
            path.parent()
                .and_then(|path| path.parent())
                .map(std::path::Path::to_path_buf)
        })
}

fn installed_enabled_map(agent_home: &std::path::Path) -> SentraResult<BTreeMap<String, bool>> {
    let Some(value) = read_json_file(agent_home.join("plugins").join("installed.json"))? else {
        return Ok(BTreeMap::new());
    };
    let mut map = BTreeMap::new();
    collect_installed_entries(&value, &mut map);
    Ok(map)
}

fn collect_installed_entries(value: &Value, map: &mut BTreeMap<String, bool>) {
    match value {
        Value::Object(raw) => {
            if let Some(enabled) = entry_enabled(raw) {
                for key in ["id", "name", "path", "root", "manifestPath"] {
                    if let Some(value) = raw.get(key).and_then(Value::as_str) {
                        map.insert(value.to_string(), enabled);
                    }
                }
            }
            for (key, value) in raw {
                if let Some(enabled) = value.as_bool() {
                    map.insert(key.clone(), enabled);
                } else if let Some(entry) = value.as_object()
                    && let Some(enabled) = entry_enabled(entry)
                {
                    map.insert(key.clone(), enabled);
                    for field in ["id", "name", "path", "root", "manifestPath"] {
                        if let Some(value) = entry.get(field).and_then(Value::as_str) {
                            map.insert(value.to_string(), enabled);
                        }
                    }
                }
                collect_installed_entries(value, map);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_installed_entries(item, map);
            }
        }
        _ => {}
    }
}

fn entry_enabled(raw: &serde_json::Map<String, Value>) -> Option<bool> {
    if raw.get("disabled").and_then(Value::as_bool) == Some(true) {
        return Some(false);
    }
    raw.get("enabled").and_then(Value::as_bool)
}

fn manifest_enabled(
    manifest: &Value,
    root: &std::path::Path,
    installed: &BTreeMap<String, bool>,
) -> bool {
    if manifest.get("disabled").and_then(Value::as_bool) == Some(true) {
        return false;
    }
    let keys = [
        string_field(manifest, "id"),
        string_field(manifest, "name"),
        root.file_name()
            .map(|name| name.to_string_lossy().to_string()),
        Some(root.display().to_string()),
    ];
    keys.into_iter()
        .flatten()
        .find_map(|key| installed.get(&key).copied())
        .or_else(|| manifest.get("enabled").and_then(Value::as_bool))
        .unwrap_or(true)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn author_name(plugin: &Value) -> Option<String> {
    plugin.get("author").and_then(|author| {
        author.as_str().map(str::to_string).or_else(|| {
            author
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
    })
}

fn capabilities(plugin: &Value) -> Vec<String> {
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
