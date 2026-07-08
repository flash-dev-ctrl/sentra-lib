use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, FileFormat, MemoryData};
use crate::utils::{
    dir_exists, get_file_size, infer_file_format, is_directory, read_json_file, read_text_file,
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
        memory_data(self.core.agent_home())
    }
}

fn memory_data(agent_home: &std::path::Path) -> SentraResult<Vec<MemoryData>> {
    let Some(memory_dir) = auto_memory_directory(agent_home)? else {
        return Ok(Vec::new());
    };
    if !dir_exists(&memory_dir) {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    walk_md_files(&memory_dir, &mut paths);
    Ok(paths.into_iter().map(memory_file).collect())
}

fn auto_memory_directory(agent_home: &std::path::Path) -> SentraResult<Option<std::path::PathBuf>> {
    for file in ["settings.json", "settings.local.json"] {
        let Some(settings) = read_json_file(agent_home.join(file))? else {
            continue;
        };
        let Some(raw) = settings
            .get("autoMemoryDirectory")
            .and_then(|value| value.as_str())
        else {
            continue;
        };
        if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
            let Some(parent) = agent_home.parent() else {
                return Ok(Some(std::path::PathBuf::from(raw)));
            };
            return Ok(Some(parent.join(rest)));
        }
        return Ok(Some(std::path::PathBuf::from(raw)));
    }
    Ok(None)
}

fn walk_md_files(dir: &std::path::Path, results: &mut Vec<std::path::PathBuf>) {
    if !dir_exists(dir) {
        return;
    }
    for entry in std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if is_directory(&path) {
            walk_md_files(&path, results);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            results.push(path);
        }
    }
}

fn memory_file(path: std::path::PathBuf) -> MemoryData {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut format = infer_file_format(&path);
    if format.is_none() && path.extension().is_none() {
        format = Some(FileFormat::Markdown);
    }
    MemoryData {
        name,
        size: get_file_size(&path),
        summary: memory_summary(&path),
        format,
        tags: memory_tags(&path),
        path,
    }
}

fn memory_summary(path: &std::path::Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy();
    let content = read_text_file(path).ok().flatten().unwrap_or_default();
    if content.is_empty() {
        let project = path
            .parent()
            .and_then(|path| path.parent())
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        return Some(format!(
            "Claude Code auto memory for project {project} (empty)"
        ));
    }
    let first = first_content_line(&content);
    Some(format!(
        "Claude Code auto memory: {}",
        if first.is_empty() {
            name.to_string()
        } else {
            first
        }
    ))
}

fn first_content_line(content: &str) -> String {
    let lines = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let mut start = 0;
    if lines.first().is_some_and(|line| line.starts_with("---")) {
        for (idx, line) in lines.iter().enumerate().skip(1) {
            if line.starts_with("---") {
                start = idx + 1;
                break;
            }
        }
    }
    lines
        .get(start)
        .map(|line| line.trim_start_matches('#').trim())
        .unwrap_or_default()
        .chars()
        .take(60)
        .collect()
}

fn memory_tags(path: &std::path::Path) -> Vec<String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mut tags = vec!["claude-code".to_string(), "auto-memory".to_string()];
    if path
        .parent()
        .and_then(|parent| parent.file_name())
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("memory"))
    {
        tags.push("project-memory".to_string());
        if let Some(project) = path
            .parent()
            .and_then(|path| path.parent())
            .and_then(|path| path.file_name())
        {
            tags.push(project.to_string_lossy().to_string());
        }
    }
    if name == "memory.md" {
        tags.push("index".to_string());
    }
    for (needle, tag) in [
        ("debug", "debugging"),
        ("api", "api"),
        ("pattern", "patterns"),
        ("convention", "conventions"),
    ] {
        if name.contains(needle) {
            tags.push(tag.to_string());
        }
    }
    tags
}
