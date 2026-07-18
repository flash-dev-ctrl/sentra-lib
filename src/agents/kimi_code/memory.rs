use std::collections::HashSet;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, FileFormat, MemoryData};
use crate::utils::{get_file_size, infer_file_format};

#[derive(Debug, Clone)]
pub(super) struct MemoryAsset {
    pub(crate) core: AssetCore,
}

impl MemoryAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(MemoryAsset, AssetType::Memory, Vec<MemoryData>);

impl Asset<Vec<MemoryData>> for MemoryAsset {
    fn get_data(&self) -> SentraResult<Vec<MemoryData>> {
        memory_data(self.core.agent_home())
    }
}

fn memory_data(agent_home: &std::path::Path) -> SentraResult<Vec<MemoryData>> {
    let mut paths = Vec::new();
    for rel in [
        "config.toml",
        "tui.toml",
        "AGENTS.md",
        "mcp.json",
        "plugins/installed.json",
        "session_index.jsonl",
        "logs/kimi-code.log",
    ] {
        push_if_file(&mut paths, agent_home.join(rel));
    }
    for manifest in crate::agents::kimi_code::plugin::plugin_manifests(agent_home)? {
        push_if_file(&mut paths, manifest.path);
    }
    collect_session_files(&agent_home.join("sessions"), 0, &mut paths);
    Ok(dedup_paths(paths)
        .into_iter()
        .map(memory_file)
        .collect::<Vec<_>>())
}

fn push_if_file(paths: &mut Vec<std::path::PathBuf>, path: std::path::PathBuf) {
    if path.is_file() {
        paths.push(path);
    }
}

fn collect_session_files(dir: &std::path::Path, depth: usize, paths: &mut Vec<std::path::PathBuf>) {
    if depth > 6 || !dir.is_dir() || skip_dir(dir) {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_session_files(&path, depth + 1, paths);
        } else if is_memory_text_file(&path) {
            paths.push(path);
        }
    }
}

fn skip_dir(path: &std::path::Path) -> bool {
    matches!(
        path.file_name()
            .and_then(|name| name.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("credentials" | "bin" | "updates")
    )
}

fn is_memory_text_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("json" | "jsonl" | "toml" | "md" | "txt" | "log")
    )
}

fn dedup_paths(paths: Vec<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for path in paths {
        if seen.insert(path.clone()) {
            results.push(path);
        }
    }
    results
}

fn memory_file(path: std::path::PathBuf) -> MemoryData {
    MemoryData {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        size: get_file_size(&path),
        summary: memory_summary(&path),
        format: infer_file_format(&path).or(Some(FileFormat::Unknown)),
        tags: memory_tags(&path),
        path,
    }
}

fn memory_summary(path: &std::path::Path) -> Option<String> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let name = path.file_name()?.to_string_lossy();
    if normalized.contains("/sessions/") {
        Some("Kimi Code session file".to_string())
    } else if normalized.ends_with("/logs/kimi-code.log") {
        Some("Kimi Code log file".to_string())
    } else if name.ends_with(".toml") || name == "mcp.json" {
        Some("Kimi Code user configuration file".to_string())
    } else if name == "AGENTS.md" {
        Some("Kimi Code user agent instruction file".to_string())
    } else if name == "installed.json" {
        Some("Kimi Code installed plugin index".to_string())
    } else if name == "kimi.plugin.json" || normalized.ends_with("/.kimi-plugin/plugin.json") {
        Some("Kimi Code plugin manifest".to_string())
    } else {
        None
    }
}

fn memory_tags(path: &std::path::Path) -> Vec<String> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mut tags = vec!["kimi-code".to_string()];
    if normalized.contains("/sessions/") || name == "session_index.jsonl" {
        tags.push("session".to_string());
    }
    if normalized.contains("/logs/") || name.ends_with(".log") {
        tags.push("log".to_string());
    }
    if matches!(
        name.as_str(),
        "config.toml" | "tui.toml" | "agents.md" | "mcp.json" | "installed.json"
    ) {
        tags.push("config".to_string());
    }
    if name == "kimi.plugin.json" || normalized.ends_with("/.kimi-plugin/plugin.json") {
        tags.push("plugin".to_string());
    }
    tags
}
