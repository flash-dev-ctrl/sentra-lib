use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, FileFormat, MemoryData};
use crate::utils::{
    collect_memory_paths, dir_exists, get_file_size, infer_file_format, is_directory,
};

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
        let home = self.core.agent_home();
        let mut results = Vec::new();
        if dir_exists(home) {
            for entry in std::fs::read_dir(home)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
            {
                let path = entry.path();
                if is_directory(&path) {
                    continue;
                }
                let ext = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(str::to_ascii_lowercase);
                if matches!(ext.as_deref(), Some("sqlite" | "db")) {
                    results.push(memory_file(path, None, memory_tags));
                }
            }
        }

        let global_state = home.join(".codex-global-state.json");
        if !is_directory(&global_state) {
            results.push(memory_file(global_state, None, memory_tags));
        }

        let memories_dir = home.join("memories");
        if dir_exists(&memories_dir) {
            let tags = vec!["codex".to_string(), "memory".to_string(), "raw".to_string()];
            for mut item in collect_memory_paths(&[memories_dir], &tags) {
                item.summary = Some("Codex memory file from memories directory".to_string());
                results.push(item);
            }
        }
        Ok(results)
    }
}

fn memory_file(
    path: std::path::PathBuf,
    summary: Option<String>,
    tags: fn(&std::path::Path) -> Vec<String>,
) -> MemoryData {
    MemoryData {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        size: get_file_size(&path),
        path: path.clone(),
        summary: summary.or_else(|| memory_summary(&path)),
        format: infer_file_format(&path).or(Some(FileFormat::Unknown)),
        tags: tags(&path),
    }
}

fn memory_summary(path: &std::path::Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if name == ".codex-global-state.json" {
        Some("Codex global state including prompt history and model preferences".to_string())
    } else if name.contains("memories") {
        Some(
            "Codex memory database containing stage1 rollout summaries and thread memories"
                .to_string(),
        )
    } else if name.contains("state") {
        Some(
            "Codex state database containing thread metadata, agent jobs, and spawn edges"
                .to_string(),
        )
    } else if name.contains("goals") {
        Some("Codex goals database containing thread objectives and token budgets".to_string())
    } else if name.contains("logs") {
        Some("Codex logs database containing operation logs and feedback".to_string())
    } else {
        None
    }
}

fn memory_tags(path: &std::path::Path) -> Vec<String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mut tags = vec!["codex".to_string()];
    if name.contains("memories") {
        tags.extend(["rollout", "summary", "memory"].map(str::to_string));
    }
    if name.contains("state") {
        tags.extend(["thread", "metadata", "state"].map(str::to_string));
    }
    if name.contains("goals") {
        tags.extend(["goal", "objective", "budget"].map(str::to_string));
    }
    if name.contains("logs") {
        tags.extend(["log", "operation", "feedback"].map(str::to_string));
    }
    if name == ".codex-global-state.json" {
        tags.extend(["global", "config", "history"].map(str::to_string));
    }
    tags
}
