use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetType, PluginData, PluginInstallSource, PluginSourceKind,
};
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
    let mut plugins = Vec::new();
    collect_cache_plugins(agent_home, &mut plugins)?;
    collect_skills_dir_plugins(agent_home, &mut plugins)?;
    Ok(dedup_plugins(plugins))
}

fn collect_cache_plugins(
    agent_home: &std::path::Path,
    plugins: &mut Vec<PluginData>,
) -> SentraResult<()> {
    let cache_dir = agent_home.join("plugins").join("cache");
    if !dir_exists(&cache_dir) {
        return Ok(());
    }
    for marketplace_dir in read_dir_paths(&cache_dir) {
        if !is_directory(&marketplace_dir) {
            continue;
        }
        let marketplace = marketplace_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string());
        for plugin_dir in read_dir_paths(&marketplace_dir) {
            if !is_directory(&plugin_dir) {
                continue;
            }
            let plugin_dir_name = plugin_dir
                .file_name()
                .map(|name| name.to_string_lossy().to_string());
            for version_dir in read_dir_paths(&plugin_dir) {
                if !is_directory(&version_dir) {
                    continue;
                }
                let manifest_path = version_dir.join(".claude-plugin").join("plugin.json");
                let Some(manifest) = read_json_file(&manifest_path)? else {
                    continue;
                };
                plugins.push(plugin_from_manifest(
                    &manifest,
                    &manifest_path,
                    &version_dir,
                    SourceContext {
                        kind: PluginSourceKind::Marketplace,
                        marketplace: marketplace.clone(),
                        origin: marketplace.clone().or_else(|| Some("cache".to_string())),
                        fallback_name: plugin_dir_name.clone(),
                        fallback_version: version_dir
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string()),
                    },
                ));
            }
        }
    }
    Ok(())
}

fn collect_skills_dir_plugins(
    agent_home: &std::path::Path,
    plugins: &mut Vec<PluginData>,
) -> SentraResult<()> {
    let skills_dir = agent_home.join("skills");
    if !dir_exists(&skills_dir) {
        return Ok(());
    }
    for plugin_root in read_dir_paths(&skills_dir) {
        if !is_directory(&plugin_root) {
            continue;
        }
        let manifest_path = plugin_root.join(".claude-plugin").join("plugin.json");
        let Some(manifest) = read_json_file(&manifest_path)? else {
            continue;
        };
        plugins.push(plugin_from_manifest(
            &manifest,
            &manifest_path,
            &plugin_root,
            SourceContext {
                kind: PluginSourceKind::LocalPath,
                marketplace: None,
                origin: Some("skills_dir".to_string()),
                fallback_name: plugin_root
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string()),
                fallback_version: None,
            },
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct SourceContext {
    kind: PluginSourceKind,
    marketplace: Option<String>,
    origin: Option<String>,
    fallback_name: Option<String>,
    fallback_version: Option<String>,
}

fn plugin_from_manifest(
    manifest: &serde_json::Value,
    manifest_path: &std::path::Path,
    plugin_root: &std::path::Path,
    context: SourceContext,
) -> PluginData {
    let name = string_field(manifest, "name")
        .or(context.fallback_name)
        .unwrap_or_else(|| "plugin".to_string());
    let version = string_field(manifest, "version").or(context.fallback_version);
    let reference = match context.kind {
        PluginSourceKind::Marketplace => marketplace_reference(
            &name,
            version.as_deref(),
            context.marketplace.as_deref(),
        )
        .unwrap_or_else(|| plugin_root.display().to_string()),
        PluginSourceKind::LocalPath => plugin_root.display().to_string(),
        _ => plugin_root.display().to_string(),
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
        origin: context.origin,
        install_source: Some(PluginInstallSource {
            kind: context.kind,
            reference,
            marketplace: context.marketplace,
        }),
        home: Some(plugin_root.to_path_buf()),
        manifest_path: Some(manifest_path.to_path_buf()),
        capabilities: capabilities(manifest),
    }
}

fn marketplace_reference(
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
        "slashCommands",
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
