use std::fs;
use std::path::Path;

use rayon::prelude::*;
use serde::Deserialize;

use crate::interfaces::{
    AssetMutationErrorCode, AssetMutationResult, SkillData, SkillFile, SkillFileType,
};
use crate::utils::{compute_content_hashes, file_mtime, get_file_size};
use crate::{SentraError, SentraResult};

pub fn collect_skill_files(
    skill_dir: impl AsRef<Path>,
    max_depth: usize,
) -> SentraResult<Vec<SkillFile>> {
    let root = skill_dir.as_ref();
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let paths = walkdir::WalkDir::new(root)
        .max_depth(max_depth + 1)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path == root || entry.file_name().to_string_lossy().starts_with('.') {
                return None;
            }
            if !entry.file_type().is_file() {
                return None;
            }
            Some(path.to_path_buf())
        })
        .collect::<Vec<_>>();

    let files = paths
        .par_iter()
        .map(|path| {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(path).unwrap_or_default();
            SkillFile {
                path: rel,
                size: get_file_size(path),
                file_type: classify_skill_file(path),
                sha256: Some(compute_content_hashes(bytes).sha256),
                mtime: file_mtime(path),
            }
        })
        .collect();
    Ok(files)
}

fn classify_skill_file(path: &Path) -> SkillFileType {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("json" | "yaml" | "yml") => SkillFileType::Config,
        Some("md") => SkillFileType::Documentation,
        Some("sh" | "py" | "js" | "ts") => SkillFileType::Script,
        Some("txt" | "prompt") => SkillFileType::Prompt,
        _ => SkillFileType::Data,
    }
}

#[derive(Debug, Deserialize, Default)]
struct SkillFrontmatter {
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    author: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

pub fn parse_skill_frontmatter(content: &str) -> SentraResult<SkillData> {
    let Some(rest) = content.strip_prefix("---") else {
        return Ok(SkillData::default());
    };
    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .unwrap_or(rest);
    let Some((frontmatter, _body)) = rest.split_once("\n---") else {
        return Ok(SkillData::default());
    };
    let fm: SkillFrontmatter = serde_yaml::from_str(frontmatter).unwrap_or_default();
    Ok(SkillData {
        id: fm.id,
        name: fm.name.unwrap_or_default(),
        description: fm.description,
        version: fm.version,
        author: fm.author,
        tags: fm.tags,
        ..SkillData::default()
    })
}

pub fn collect_skills_from_dir(base_dir: impl AsRef<Path>) -> SentraResult<Vec<SkillData>> {
    collect_skills_from_dir_inner(base_dir, true)
}

pub async fn collect_skills_from_dir_async(
    base_dir: impl AsRef<Path>,
) -> SentraResult<Vec<SkillData>> {
    let base_dir = base_dir.as_ref().to_path_buf();
    tokio::task::spawn_blocking(move || collect_skills_from_dir(base_dir))
        .await
        .map_err(|err| SentraError::Message(err.to_string()))?
}

pub fn collect_skill_manifests_from_dir(
    base_dir: impl AsRef<Path>,
) -> SentraResult<Vec<SkillData>> {
    collect_skills_from_dir_inner(base_dir, false)
}

pub async fn collect_skill_manifests_from_dir_async(
    base_dir: impl AsRef<Path>,
) -> SentraResult<Vec<SkillData>> {
    let base_dir = base_dir.as_ref().to_path_buf();
    tokio::task::spawn_blocking(move || collect_skill_manifests_from_dir(base_dir))
        .await
        .map_err(|err| SentraError::Message(err.to_string()))?
}

fn collect_skills_from_dir_inner(
    base_dir: impl AsRef<Path>,
    include_files: bool,
) -> SentraResult<Vec<SkillData>> {
    let base_dir = base_dir.as_ref();
    let mut results = Vec::new();
    if !base_dir.is_dir() {
        return Ok(results);
    }
    let base_dir = clean_canonical_path(
        base_dir
            .canonicalize()
            .map_err(|err| SentraError::io(Some(base_dir.to_path_buf()), err))?,
    );

    collect_skills_recursive(&base_dir, &base_dir, &mut results, include_files)?;
    Ok(results)
}

fn collect_skills_recursive(
    base_dir: &Path,
    dir: &Path,
    results: &mut Vec<SkillData>,
    include_files: bool,
) -> SentraResult<()> {
    let skill_md = dir.join("SKILL.md");
    if skill_md.is_file() {
        let content = fs::read_to_string(&skill_md)
            .map_err(|err| SentraError::io(Some(skill_md.clone()), err))?;
        let mut skill = parse_skill_frontmatter(&content)?;
        let rel = dir.strip_prefix(base_dir).unwrap_or(dir);
        let fallback_name = rel
            .file_name()
            .or_else(|| dir.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if skill.name.is_empty() {
            skill.name = fallback_name;
        }
        skill.enabled = Some(true);
        skill.home = Some(dir.to_path_buf());
        if include_files {
            skill.files = collect_skill_files(dir, 12)?;
        }
        skill.id = if include_files {
            let mut hashes = skill
                .files
                .iter()
                .filter_map(|file| file.sha256.as_deref())
                .collect::<Vec<_>>();
            hashes.sort_unstable();
            let hashes = hashes.join("");
            if hashes.is_empty() {
                Some(rel.to_string_lossy().replace('\\', "/"))
            } else {
                Some(compute_content_hashes(hashes).sha256)
            }
        } else {
            Some(rel.to_string_lossy().replace('\\', "/"))
        };
        results.push(skill);
        return Ok(());
    }

    for entry in fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        if !should_descend_skill_dir(&entry.file_name()) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_skills_recursive(base_dir, &path, results, include_files)?;
        }
    }
    Ok(())
}

fn should_descend_skill_dir(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    !name.starts_with('.') || name == ".system"
}

pub fn set_skill_data(skills_dir: &Path, skill_path: &Path) -> SentraResult<AssetMutationResult> {
    fs::create_dir_all(skills_dir).map_err(|err| SentraError::io(Some(skills_dir.into()), err))?;
    let skill_name = skill_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dest = skills_dir.join(&skill_name);
    if normalize(&dest) == normalize(skill_path) {
        return Ok(AssetMutationResult {
            changed: false,
            errors: Vec::new(),
        });
    }
    copy_dir_all(skill_path, &dest)?;
    Ok(AssetMutationResult::changed())
}

pub fn del_skill_data(skill_path: &Path) -> SentraResult<AssetMutationResult> {
    if !skill_path.exists() {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::NotFound,
            format!("Skill path does not exist: {}", skill_path.display()),
        ));
    }
    fs::remove_dir_all(skill_path)
        .map_err(|err| SentraError::io(Some(skill_path.to_path_buf()), err))?;
    Ok(AssetMutationResult::changed())
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> SentraResult<()> {
    if dst.exists() {
        fs::remove_dir_all(dst).map_err(|err| SentraError::io(Some(dst.to_path_buf()), err))?;
    }
    fs::create_dir_all(dst).map_err(|err| SentraError::io(Some(dst.to_path_buf()), err))?;
    for entry in walkdir::WalkDir::new(src)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let rel = path.strip_prefix(src).unwrap_or(path);
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest).map_err(|err| SentraError::io(Some(dest), err))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .map_err(|err| SentraError::io(Some(parent.to_path_buf()), err))?;
            }
            fs::copy(path, &dest).map_err(|err| SentraError::io(Some(dest), err))?;
        }
    }
    Ok(())
}

fn normalize(path: &Path) -> std::path::PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn clean_canonical_path(path: std::path::PathBuf) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
            return std::path::PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = value.strip_prefix(r"\\?\") {
            return std::path::PathBuf::from(rest);
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::{collect_skill_manifests_from_dir, collect_skills_from_dir};

    #[test]
    fn manifest_collection_skips_file_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("skills").join("demo");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo\n---\nbody",
        )
        .unwrap();
        std::fs::write(skill_dir.join("script.py"), "print('hello')").unwrap();

        let full = collect_skills_from_dir(dir.path()).unwrap();
        let manifest = collect_skill_manifests_from_dir(dir.path()).unwrap();

        assert_eq!(full.len(), 1);
        assert_eq!(manifest.len(), 1);
        assert!(!full[0].files.is_empty());
        assert!(manifest[0].files.is_empty());
        assert_eq!(manifest[0].name, "demo");
    }

    #[test]
    fn collection_includes_codex_system_skills() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir
            .path()
            .join("skills")
            .join(".system")
            .join("openai-docs");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: openai-docs\ndescription: Docs\n---\nbody",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("skills").join(".cache")).unwrap();
        std::fs::write(
            dir.path().join("skills").join(".cache").join("SKILL.md"),
            "---\nname: hidden-cache\n---\nbody",
        )
        .unwrap();

        let skills = collect_skills_from_dir(dir.path().join("skills")).unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "openai-docs");
        assert_eq!(skills[0].home.as_deref(), Some(skill_dir.as_path()),);
    }
}
