use std::collections::BTreeSet;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, FileFormat, MemoryData};
use crate::utils::{collect_memory_paths, infer_file_format};

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
        Ok(memory_data(self.core.agent_home()))
    }
}

fn memory_data(agent_home: &std::path::Path) -> Vec<MemoryData> {
    let data_home = crate::agents::opencode::data_home(agent_home);
    let mut paths = vec![
        data_home.join("auth.json"),
        data_home.join("opencode.db"),
        data_home.join("opencode.db-shm"),
        data_home.join("opencode.db-wal"),
        data_home.join("log").join("opencode.log"),
        data_home.join("tool-output"),
        data_home.join("snapshot"),
        data_home.join("repos"),
    ];
    paths.extend(crate::agents::opencode::config_files(agent_home));
    paths.push(agent_home.join("auth.json"));
    for name in [
        "agent", "agents", "command", "commands", "plugin", "plugins", "rule", "rules",
    ] {
        paths.push(agent_home.join(name));
    }

    let tags = vec!["opencode".to_string(), "memory".to_string()];
    let mut seen = BTreeSet::new();
    collect_memory_paths(&paths, &tags)
        .into_iter()
        .filter(|item| seen.insert(item.path.clone()))
        .map(|mut item| {
            item.summary = memory_summary(&item.path);
            item.format = infer_file_format(&item.path).or(Some(FileFormat::Unknown));
            item.tags = memory_tags(&item.path);
            item
        })
        .collect()
}

fn memory_summary(path: &std::path::Path) -> Option<String> {
    let value = path.to_string_lossy().to_ascii_lowercase();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name == "opencode.json" {
        Some("OpenCode configuration including providers, model, plugins, MCP servers, commands, rules, and agent settings".to_string())
    } else if name == "auth.json" {
        Some("OpenCode auth metadata and credentials file; Sentra asset collection does not parse secret values into memory data".to_string())
    } else if name.starts_with("opencode.db") {
        Some("OpenCode SQLite state database containing projects, sessions, messages, events, permissions, and local metadata".to_string())
    } else if name == "opencode.log" {
        Some("OpenCode local log file".to_string())
    } else if value.contains("tool-output") {
        Some("OpenCode tool output artifact".to_string())
    } else if value.contains("snapshot") {
        Some("OpenCode repository snapshot data".to_string())
    } else if value.contains("repos") {
        Some("OpenCode local repository data".to_string())
    } else if value.contains("agent") {
        Some("OpenCode agent configuration or prompt file".to_string())
    } else if value.contains("command") {
        Some("OpenCode custom command configuration or prompt file".to_string())
    } else if value.contains("plugin") {
        Some("OpenCode plugin configuration file".to_string())
    } else if value.contains("rule") {
        Some("OpenCode rule configuration file".to_string())
    } else {
        None
    }
}

fn memory_tags(path: &std::path::Path) -> Vec<String> {
    let value = path.to_string_lossy().to_ascii_lowercase();
    let mut tags = vec!["opencode".to_string(), "memory".to_string()];
    if value.contains("opencode.json") || value.contains("agent") || value.contains("command") {
        tags.push("config".to_string());
        tags.push("prompt".to_string());
    }
    if value.contains("auth.json") {
        tags.push("auth".to_string());
    }
    if value.contains("opencode.db") {
        tags.push("database".to_string());
        tags.push("session".to_string());
    }
    if value.contains("log") {
        tags.push("log".to_string());
    }
    if value.contains("tool-output") {
        tags.push("tool-output".to_string());
    }
    if value.contains("snapshot") {
        tags.push("snapshot".to_string());
    }
    if value.contains("repos") {
        tags.push("repo".to_string());
    }
    if value.contains("plugin") {
        tags.push("plugin".to_string());
    }
    if value.contains("rule") {
        tags.push("rule".to_string());
    }
    tags
}
