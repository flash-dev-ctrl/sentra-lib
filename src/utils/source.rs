use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use crate::{
    SentraError, SentraResult,
    utils::{download_url_to_file, extract_zip_to_dir, url_file_name},
};

pub struct StagedSkillSource {
    path: PathBuf,
    _temp_dir: Option<TempDir>,
}

impl StagedSkillSource {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn stage_skill_source(source: &str) -> SentraResult<StagedSkillSource> {
    if is_git_source(source) {
        return stage_git_source(source);
    }
    if is_http_url(source) {
        return stage_remote_source(source);
    }
    if let Some(path) = source.strip_prefix("file://") {
        return stage_local_path(&file_url_path(path));
    }
    stage_local_path(Path::new(source))
}

fn file_url_path(path: &str) -> PathBuf {
    #[cfg(windows)]
    {
        let trimmed = path.strip_prefix('/').unwrap_or(path);
        if trimmed.as_bytes().get(1) == Some(&b':') {
            return PathBuf::from(trimmed);
        }
    }
    PathBuf::from(path)
}

fn stage_local_path(path: &Path) -> SentraResult<StagedSkillSource> {
    if path.is_dir() {
        return Ok(StagedSkillSource {
            path: path.to_path_buf(),
            _temp_dir: None,
        });
    }
    if !path.is_file() {
        return Err(SentraError::Message(format!(
            "skill source does not exist: {}",
            path.display()
        )));
    }
    if is_zip_path(path) {
        let temp_dir = tempfile::tempdir().map_err(|err| SentraError::io(None, err))?;
        let extract_dir = temp_dir.path().join("source");
        extract_zip_to_dir(path, &extract_dir)?;
        return Ok(StagedSkillSource {
            path: extract_dir,
            _temp_dir: Some(temp_dir),
        });
    }
    stage_single_skill_file(path)
}

fn stage_remote_source(url: &str) -> SentraResult<StagedSkillSource> {
    let temp_dir = tempfile::tempdir().map_err(|err| SentraError::io(None, err))?;
    let file_name = url_file_name(url);
    let download_path = temp_dir.path().join(if file_name.is_empty() {
        "skill-source".to_string()
    } else {
        file_name
    });
    download_url_to_file(url, &download_path)?;
    if is_zip_path(&download_path) {
        let extract_dir = temp_dir.path().join("source");
        extract_zip_to_dir(&download_path, &extract_dir)?;
        Ok(StagedSkillSource {
            path: extract_dir,
            _temp_dir: Some(temp_dir),
        })
    } else {
        let skill_dir = temp_dir.path().join("source").join("downloaded-skill");
        std::fs::create_dir_all(&skill_dir)
            .map_err(|err| SentraError::io(Some(skill_dir.clone()), err))?;
        std::fs::copy(&download_path, skill_dir.join("SKILL.md"))
            .map_err(|err| SentraError::io(Some(download_path), err))?;
        Ok(StagedSkillSource {
            path: temp_dir.path().join("source"),
            _temp_dir: Some(temp_dir),
        })
    }
}

fn stage_git_source(source: &str) -> SentraResult<StagedSkillSource> {
    let url = source.strip_prefix("git+").unwrap_or(source);
    let temp_dir = tempfile::tempdir().map_err(|err| SentraError::io(None, err))?;
    let checkout_dir = temp_dir.path().join("repo");
    let status = Command::new("git")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["clone", "--depth", "1", "--progress", url])
        .arg(&checkout_dir)
        .status()
        .map_err(|err| SentraError::Message(format!("failed to run git clone: {err}")))?;
    if !status.success() {
        return Err(SentraError::Message(format!(
            "failed to clone git source {url}: git exited with {status}"
        )));
    }
    Ok(StagedSkillSource {
        path: checkout_dir,
        _temp_dir: Some(temp_dir),
    })
}

fn stage_single_skill_file(path: &Path) -> SentraResult<StagedSkillSource> {
    let temp_dir = tempfile::tempdir().map_err(|err| SentraError::io(None, err))?;
    let skill_name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("skill");
    let skill_dir = temp_dir.path().join("source").join(skill_name);
    std::fs::create_dir_all(&skill_dir)
        .map_err(|err| SentraError::io(Some(skill_dir.clone()), err))?;
    std::fs::copy(path, skill_dir.join("SKILL.md"))
        .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))?;
    Ok(StagedSkillSource {
        path: temp_dir.path().join("source"),
        _temp_dir: Some(temp_dir),
    })
}

fn is_http_url(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

fn is_git_source(source: &str) -> bool {
    source.starts_with("git+")
        || source.ends_with(".git")
        || source.starts_with("git@")
        || source.starts_with("ssh://")
        || is_github_repository_url(source)
}

fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}

fn is_github_repository_url(source: &str) -> bool {
    let Some(rest) = source
        .strip_prefix("https://github.com/")
        .or_else(|| source.strip_prefix("http://github.com/"))
    else {
        return false;
    };
    let path = rest
        .split(['?', '#'])
        .next()
        .unwrap_or(rest)
        .trim_matches('/');
    let segments = path.split('/').collect::<Vec<_>>();
    if segments.len() != 2 {
        return false;
    }
    let repo = segments[1];
    !segments[0].is_empty()
        && !repo.is_empty()
        && !repo.ends_with(".zip")
        && !repo.ends_with(".tar.gz")
}

#[cfg(test)]
mod tests {
    use super::is_git_source;

    #[test]
    fn github_repository_url_without_dot_git_is_a_git_source() {
        assert!(is_git_source("https://github.com/obra/superpowers"));
    }
}
