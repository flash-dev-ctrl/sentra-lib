use std::collections::HashSet;

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, SkillData};
use crate::utils::collect_skills_from_dir;

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
            "Kimi Code skill mutation is not supported",
        ))
    }

    fn del_data(&self, _item: &SkillData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Kimi Code skill mutation is not supported",
        ))
    }
}

pub(super) fn skill_data(agent_home: &std::path::Path) -> SentraResult<Vec<SkillData>> {
    let mut results = collect_skills_from_dir(agent_home.join("skills"))?;
    if let Some(user_home) = crate::agents::kimi_code::default_user_home(agent_home) {
        results.extend(collect_skills_from_dir(
            user_home.join(".agents").join("skills"),
        )?);
    }
    results.extend(plugin_skill_data(agent_home)?);
    Ok(dedup_skills(results))
}

pub(super) fn plugin_skill_data(agent_home: &std::path::Path) -> SentraResult<Vec<SkillData>> {
    let mut results = Vec::new();
    for manifest in crate::agents::kimi_code::plugin::plugin_manifests(agent_home)? {
        if !manifest.enabled {
            continue;
        }
        for skills_dir in skill_dirs(&manifest.value, &manifest.root) {
            let author = author_name(&manifest.value);
            let source = string_field(&manifest.value, "name");
            let version = string_field(&manifest.value, "version");
            for mut skill in collect_skills_from_dir(skills_dir)? {
                skill.source = skill.source.or_else(|| source.clone());
                skill.author = skill.author.or_else(|| author.clone());
                skill.version = skill.version.or_else(|| version.clone());
                results.push(skill);
            }
        }
    }
    Ok(results)
}

fn skill_dirs(manifest: &Value, root: &std::path::Path) -> Vec<std::path::PathBuf> {
    match manifest.get("skills") {
        Some(Value::String(path)) => vec![root.join(path)],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(|path| root.join(path))
            .collect(),
        _ => Vec::new(),
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn author_name(plugin: &Value) -> Option<String> {
    plugin.get("author").and_then(|author| {
        author.as_str().map(str::to_string).or_else(|| {
            author
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
    })
}

pub(super) fn dedup_skills(skills: Vec<SkillData>) -> Vec<SkillData> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for skill in skills {
        if skill
            .home
            .as_ref()
            .map(|home| seen.insert(home.clone()))
            .unwrap_or(true)
        {
            results.push(skill);
        }
    }
    results
}
