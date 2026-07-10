use std::collections::HashSet;
use std::fs;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, SkillData, SkillFile,
    SkillFileType,
};
use crate::utils::{
    collect_skills_from_dir, compute_content_hashes, file_mtime, get_file_size,
    parse_skill_frontmatter,
};

#[derive(Debug, Clone)]
pub(super) struct SkillAsset {
    pub(crate) core: AssetCore,
}

impl SkillAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(SkillAsset, AssetType::Skill, Vec<SkillData>, SkillData);

impl Asset<Vec<SkillData>, SkillData> for SkillAsset {
    fn get_data(&self) -> SentraResult<Vec<SkillData>> {
        skill_data(self.core.agent_home())
    }

    fn set_data(&self, _value: SkillData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "OpenCode skill mutation is not supported",
        ))
    }

    fn del_data(&self, _item: &SkillData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "OpenCode skill mutation is not supported",
        ))
    }
}

fn skill_data(agent_home: &std::path::Path) -> SentraResult<Vec<SkillData>> {
    let mut results = Vec::new();
    for home in crate::agents::opencode::config_homes(agent_home) {
        for dir in [home.join("skill"), home.join("skills")] {
            results.extend(collect_single_file_skills(&dir)?);
            results.extend(collect_skills_from_dir(&dir)?);
        }
    }
    let mut seen = HashSet::new();
    results.retain(|skill| {
        skill
            .home
            .as_ref()
            .map(|home| seen.insert(home.clone()))
            .unwrap_or(true)
    });
    Ok(results)
}

fn collect_single_file_skills(base_dir: &std::path::Path) -> SentraResult<Vec<SkillData>> {
    if !base_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut results = Vec::new();
    for entry in walkdir::WalkDir::new(base_dir)
        .max_depth(8)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
            continue;
        }
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| !ext.eq_ignore_ascii_case("md"))
            .unwrap_or(true)
        {
            continue;
        }
        let content = fs::read_to_string(path)
            .map_err(|err| crate::SentraError::io(Some(path.into()), err))?;
        let mut skill = parse_skill_frontmatter(&content)?;
        if skill.name.is_empty() {
            skill.name = path
                .file_stem()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
        }
        skill.enabled = Some(true);
        skill.home = Some(path.to_path_buf());
        let hashes = compute_content_hashes(content.as_bytes());
        skill.id = skill.id.or_else(|| Some(hashes.sha256.clone()));
        skill.files = vec![SkillFile {
            path: path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            size: get_file_size(path),
            file_type: SkillFileType::Documentation,
            sha256: Some(hashes.sha256),
            mtime: file_mtime(path),
        }];
        if !skill.tags.iter().any(|tag| tag == "opencode") {
            skill.tags.push("opencode".to_string());
        }
        results.push(skill);
    }
    Ok(results)
}
