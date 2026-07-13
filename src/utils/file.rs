use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::interfaces::{FileCategory, FileExtType, FileFormat};
use crate::{SentraError, SentraResult};

pub fn dir_exists(path: impl AsRef<Path>) -> bool {
    fs::metadata(path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false)
}

pub fn is_directory(path: impl AsRef<Path>) -> bool {
    dir_exists(path)
}

pub fn read_text_file(path: impl AsRef<Path>) -> SentraResult<Option<String>> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(None);
    }
    fs::read_to_string(path)
        .map(Some)
        .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))
}

pub fn read_json_file(path: impl AsRef<Path>) -> SentraResult<Option<serde_json::Value>> {
    let Some(content) = read_text_file(path)? else {
        return Ok(None);
    };
    serde_json::from_str(&content).map(Some).map_err(Into::into)
}

pub fn write_text_file(path: impl AsRef<Path>, content: &str) -> SentraResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| SentraError::io(Some(parent.to_path_buf()), err))?;
    }
    fs::write(path, content).map_err(|err| SentraError::io(Some(path.to_path_buf()), err))
}

pub fn write_json_file(path: impl AsRef<Path>, data: &serde_json::Value) -> SentraResult<()> {
    write_text_file(path, &serde_json::to_string_pretty(data)?)
}

pub fn backup_file(path: impl AsRef<Path>) -> SentraResult<Option<PathBuf>> {
    let path = path.as_ref();
    if !path.is_file() {
        return Ok(None);
    }
    let stamp = chrono::Utc::now().to_rfc3339().replace([':', '.'], "-");
    for index in 0..1000 {
        let suffix = if index == 0 {
            format!("bak.{stamp}")
        } else {
            format!("bak.{stamp}.{index}")
        };
        let backup = path.with_extension(format!(
            "{}.{suffix}",
            path.extension().and_then(|ext| ext.to_str()).unwrap_or("")
        ));
        if backup.exists() {
            continue;
        }
        fs::copy(path, &backup).map_err(|err| SentraError::io(Some(path.to_path_buf()), err))?;
        return Ok(Some(backup));
    }
    Err(SentraError::Message(
        "unable to create unique backup path".to_string(),
    ))
}

pub fn get_file_size(path: impl AsRef<Path>) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

pub fn file_mtime(path: impl AsRef<Path>) -> f64 {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

pub fn infer_file_format(path: impl AsRef<Path>) -> Option<FileFormat> {
    match path
        .as_ref()
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("json") => Some(FileFormat::Json),
        Some("yaml" | "yml") => Some(FileFormat::Yaml),
        Some("toml") => Some(FileFormat::Toml),
        Some("xml") => Some(FileFormat::Xml),
        Some("csv") => Some(FileFormat::Csv),
        Some("txt") => Some(FileFormat::Txt),
        Some("md" | "markdown") => Some(FileFormat::Markdown),
        Some("sqlite" | "db") => Some(FileFormat::Sqlite),
        _ => None,
    }
}

pub fn is_text_file(path: impl AsRef<Path>) -> bool {
    matches!(
        path.as_ref()
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some(
            "md" | "markdown"
                | "txt"
                | "json"
                | "yaml"
                | "yml"
                | "toml"
                | "js"
                | "ts"
                | "py"
                | "sh"
                | "ps1"
                | "bat"
                | "cmd"
                | "rs"
                | "go"
                | "java"
                | "c"
                | "cpp"
                | "h"
                | "hpp"
                | "xml"
                | "csv"
                | "sql"
                | "yar"
                | "yara"
        )
    )
}

pub fn resolve_content_meta(
    source: impl AsRef<Path>,
    content: &str,
    existing: Option<(Option<FileCategory>, Option<FileExtType>)>,
) -> (FileCategory, FileExtType) {
    let source = source.as_ref();
    let ext = existing
        .and_then(|(_, ext)| ext)
        .unwrap_or_else(|| infer_ext_type(source));
    let cat = existing
        .and_then(|(cat, _)| cat)
        .unwrap_or_else(|| infer_category(source, content, ext));
    (cat, ext)
}

fn infer_ext_type(path: &Path) -> FileExtType {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("md" | "markdown") => FileExtType::Md,
        Some("json") => FileExtType::Json,
        Some("yaml" | "yml") => FileExtType::Yaml,
        Some("js") => FileExtType::Js,
        Some("ts") => FileExtType::Ts,
        Some("py") => FileExtType::Py,
        Some("sh") => FileExtType::Sh,
        Some("ps1") => FileExtType::Ps1,
        Some("bat" | "cmd") => FileExtType::Bat,
        _ => FileExtType::Unknown,
    }
}

fn infer_category(path: &Path, content: &str, ext: FileExtType) -> FileCategory {
    match ext {
        FileExtType::Md | FileExtType::Json | FileExtType::Yaml => FileCategory::Prompt,
        FileExtType::Js
        | FileExtType::Ts
        | FileExtType::Py
        | FileExtType::Sh
        | FileExtType::Ps1
        | FileExtType::Bat => FileCategory::Script,
        FileExtType::Unknown => {
            if is_text_file(path) || !content.is_empty() {
                FileCategory::Prompt
            } else {
                FileCategory::Unknown
            }
        }
    }
}

pub fn truncate_content(content: &str, max_chars: usize) -> String {
    content.chars().take(max_chars).collect()
}

pub fn url_file_name(url: &str) -> String {
    url.rsplit('/')
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or("rules")
        .split(['?', '#'])
        .next()
        .unwrap_or("rules")
        .to_string()
}

pub fn download_url_to_file(url: &str, dest: impl AsRef<Path>) -> SentraResult<()> {
    let dest = dest.as_ref();
    let response = ureq::get(url)
        .call()
        .map_err(|err| SentraError::Message(format!("failed to download {url}: {err}")))?;
    let mut reader = response.into_reader();
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| SentraError::io(Some(parent.to_path_buf()), err))?;
    }
    let mut file =
        fs::File::create(dest).map_err(|err| SentraError::io(Some(dest.to_path_buf()), err))?;
    io::copy(&mut reader, &mut file)
        .map_err(|err| SentraError::io(Some(dest.to_path_buf()), err))?;
    Ok(())
}

pub fn extract_zip_to_dir(zip_path: impl AsRef<Path>, dest: impl AsRef<Path>) -> SentraResult<()> {
    let zip_path = zip_path.as_ref();
    let dest = dest.as_ref();
    let file = fs::File::open(zip_path)
        .map_err(|err| SentraError::io(Some(zip_path.to_path_buf()), err))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|err| SentraError::Message(err.to_string()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| SentraError::Message(err.to_string()))?;
        let Some(enclosed) = entry.enclosed_name().map(|name| name.to_path_buf()) else {
            continue;
        };
        let out_path = dest.join(enclosed);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|err| SentraError::io(Some(out_path.clone()), err))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| SentraError::io(Some(parent.to_path_buf()), err))?;
        }
        let mut out_file = fs::File::create(&out_path)
            .map_err(|err| SentraError::io(Some(out_path.clone()), err))?;
        io::copy(&mut entry, &mut out_file).map_err(|err| SentraError::io(Some(out_path), err))?;
    }
    Ok(())
}

pub fn is_path_inside(parent: impl AsRef<Path>, child: impl AsRef<Path>) -> bool {
    let parent = parent
        .as_ref()
        .canonicalize()
        .unwrap_or_else(|_| parent.as_ref().to_path_buf());
    let child = child
        .as_ref()
        .canonicalize()
        .unwrap_or_else(|_| child.as_ref().to_path_buf());
    child.starts_with(parent)
}

pub fn mask_secret(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    let chars = value.chars().collect::<Vec<_>>();
    let keep = if chars.len() <= 8 { 2 } else { 4 };
    let prefix = chars.iter().take(keep).collect::<String>();
    let suffix = chars
        .iter()
        .skip(chars.len().saturating_sub(keep))
        .collect::<String>();
    Some(format!("{prefix}****{suffix}"))
}
