use std::collections::HashSet;

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

fn plugin_data(agent_home: &std::path::Path) -> SentraResult<Vec<PluginData>> {
    let mut plugins = Vec::new();
    collect_config_plugins(agent_home, &mut plugins)?;
    collect_local_plugins(agent_home, &mut plugins);
    Ok(dedup_plugins(plugins))
}

fn collect_config_plugins(
    agent_home: &std::path::Path,
    plugins: &mut Vec<PluginData>,
) -> SentraResult<()> {
    for config_file in super::config_files(agent_home) {
        let Some(config) = read_json_file(&config_file)? else {
            continue;
        };
        let Some(items) = config.get("plugin").and_then(|value| value.as_array()) else {
            continue;
        };
        for item in items {
            let Some(reference) = item
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let name = plugin_name_from_reference(reference);
            let kind = source_kind_from_reference(reference);
            plugins.push(PluginData {
                id: Some(reference.to_string()),
                name,
                display_name: None,
                description: None,
                version: None,
                author: None,
                enabled: Some(true),
                origin: Some("config".to_string()),
                install_source: Some(PluginInstallSource {
                    kind,
                    reference: reference.to_string(),
                    marketplace: None,
                }),
                home: None,
                manifest_path: Some(config_file.clone()),
                capabilities: vec!["runtime".to_string()],
            });
        }
    }
    Ok(())
}

fn collect_local_plugins(agent_home: &std::path::Path, plugins: &mut Vec<PluginData>) {
    let user_home = super::user_home(agent_home);
    let mut dirs = vec![agent_home.join("plugins")];
    let legacy = user_home.join(".opencode").join("plugins");
    if !dirs.iter().any(|dir| dir == &legacy) {
        dirs.push(legacy);
    }
    for dir in dirs {
        for path in plugin_files(&dir, 4, 0) {
            let name = path
                .file_stem()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "plugin".to_string());
            plugins.push(PluginData {
                id: Some(path.to_string_lossy().to_string()),
                name,
                display_name: None,
                description: None,
                version: None,
                author: None,
                enabled: Some(true),
                origin: Some("local_path".to_string()),
                install_source: Some(PluginInstallSource {
                    kind: PluginSourceKind::LocalPath,
                    reference: path.to_string_lossy().to_string(),
                    marketplace: None,
                }),
                home: Some(path),
                manifest_path: None,
                capabilities: vec!["runtime".to_string()],
            });
        }
    }
}

fn plugin_files(dir: &std::path::Path, max_depth: usize, depth: usize) -> Vec<std::path::PathBuf> {
    if depth > max_depth || !dir_exists(dir) {
        return Vec::new();
    }
    let mut results = Vec::new();
    for entry in read_dir_paths(dir) {
        if is_directory(&entry) {
            results.extend(plugin_files(&entry, max_depth, depth + 1));
            continue;
        }
        if is_plugin_file(&entry) {
            results.push(entry);
        }
    }
    results
}

fn is_plugin_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("js" | "ts" | "mjs" | "cjs")
    )
}

fn source_kind_from_reference(reference: &str) -> PluginSourceKind {
    if reference.contains("@git+")
        || reference.starts_with("git+")
        || reference.ends_with(".git")
        || reference.contains("github.com/")
    {
        return PluginSourceKind::Git;
    }
    if looks_like_local_path(reference) {
        return PluginSourceKind::LocalPath;
    }
    PluginSourceKind::Npm
}

fn looks_like_local_path(reference: &str) -> bool {
    reference.starts_with('.')
        || reference.starts_with('/')
        || reference.starts_with('~')
        || reference.contains('\\')
        || reference.chars().nth(1).is_some_and(|ch| ch == ':')
}

fn plugin_name_from_reference(reference: &str) -> String {
    if let Some((name, _)) = reference.split_once("@git+") {
        return name.trim_matches('@').to_string();
    }
    if looks_like_local_path(reference) {
        return std::path::Path::new(reference)
            .file_stem()
            .map(|name| name.to_string_lossy().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "plugin".to_string());
    }
    npm_package_name(reference)
}

fn npm_package_name(reference: &str) -> String {
    if reference.starts_with('@') {
        let Some(slash) = reference.find('/') else {
            return reference.to_string();
        };
        let package = &reference[slash + 1..];
        if let Some(version_index) = package.find('@') {
            return reference[..slash + 1 + version_index].to_string();
        }
        return reference.to_string();
    }
    reference
        .split('@')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(reference)
        .to_string()
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
