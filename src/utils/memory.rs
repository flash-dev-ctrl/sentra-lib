use std::fs;
use std::path::{Path, PathBuf};

use crate::interfaces::MemoryData;
use crate::utils::{get_file_size, infer_file_format};

pub fn collect_memory_paths(paths: &[PathBuf], tags: &[String]) -> Vec<MemoryData> {
    let mut files = Vec::new();
    for path in paths {
        collect_memory_path(path, 0, &mut files);
    }
    files
        .into_iter()
        .map(|path| MemoryData {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            size: get_file_size(&path),
            format: infer_file_format(&path),
            path,
            summary: None,
            tags: tags.to_vec(),
        })
        .collect()
}

fn collect_memory_path(path: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    if depth > 6 {
        return;
    }
    if path.is_dir() {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            collect_memory_path(&entry.path(), depth + 1, files);
        }
    } else if path.exists() {
        files.push(path.to_path_buf());
    }
}
