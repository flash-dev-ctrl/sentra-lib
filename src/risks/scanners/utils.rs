use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::interfaces::{CheckInput, FileCategory, FileExtType};
use crate::utils::{compute_content_hashes, resolve_content_meta};
use crate::{SentraError, SentraResult};

const SKIP_DIRS: &[&str] = &[".git", "node_modules", ".venv", "dist", "build"];
const MAX_SCAN_CHARS: usize = 50_000;

pub(crate) fn path_content_inputs(path: &Path, text_only: bool) -> SentraResult<Vec<CheckInput>> {
    let files = discover_files(path, text_only)?;
    let scanned = files
        .par_iter()
        .map(|file| {
            read_file_for_scan(file, text_only)
                .map(|value| value.map(|(raw, content)| build_content_input(file, content, &raw)))
        })
        .collect::<Vec<_>>();

    let mut inputs = Vec::with_capacity(scanned.len());
    for item in scanned {
        if let Some(input) = item? {
            inputs.push(input);
        }
    }
    Ok(inputs)
}

pub(crate) fn build_content_input(source: &Path, content: String, raw: &[u8]) -> CheckInput {
    let truncated = crate::utils::truncate_content(&content, MAX_SCAN_CHARS);
    let (cat, ext) = resolve_content_meta(source, &truncated, None);
    CheckInput::content(source.to_string_lossy(), truncated)
        .with_file_meta(cat, ext)
        .with_hashes(compute_content_hashes(raw))
}

pub(crate) fn build_prompt_input(source: &str, content: &str) -> CheckInput {
    let truncated = crate::utils::truncate_content(content, MAX_SCAN_CHARS);
    CheckInput::content(source, truncated)
        .with_file_meta(FileCategory::Prompt, FileExtType::Unknown)
        .with_hashes(compute_content_hashes(content.as_bytes()))
}

fn discover_files(path: &Path, text_only: bool) -> SentraResult<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Err(SentraError::Message(format!(
            "scan path does not exist: {}",
            path.display()
        )));
    }
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_entry(|entry| !SKIP_DIRS.contains(&entry.file_name().to_string_lossy().as_ref()))
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if text_only && !crate::utils::is_text_file(entry.path()) {
            continue;
        }
        files.push(entry.path().to_path_buf());
    }
    Ok(files)
}

fn read_file_for_scan(path: &Path, text_only: bool) -> SentraResult<Option<(Vec<u8>, String)>> {
    if text_only && !crate::utils::is_text_file(path) {
        return Ok(None);
    }
    let raw = fs::read(path).map_err(|err| SentraError::io(Some(path.to_path_buf()), err))?;
    let content = if crate::utils::is_text_file(path) {
        String::from_utf8_lossy(&raw).to_string()
    } else {
        String::new()
    };
    Ok(Some((raw, content)))
}
