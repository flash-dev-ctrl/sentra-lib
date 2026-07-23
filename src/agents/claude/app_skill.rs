use std::sync::{Arc, Mutex};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, SkillData};
use crate::utils::{
    backup_file, collect_skills_from_dir, del_skill_data, dir_exists, is_directory, read_json_file,
    set_skill_data, write_json_file,
};

#[derive(Debug, Clone)]
pub(super) struct SkillAsset {
    pub(crate) core: AssetCore,
    cache: Arc<Mutex<Option<Vec<SkillData>>>>,
}

impl SkillAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
            cache: Arc::new(Mutex::new(None)),
        }
    }
}

impl_erased_asset!(SkillAsset, AssetType::Skill, Vec<SkillData>, SkillData);

impl Asset<Vec<SkillData>, SkillData> for SkillAsset {
    fn get_data(&self) -> SentraResult<Vec<SkillData>> {
        if let Some(cached) = self.cache.lock().unwrap().clone() {
            return Ok(cached);
        }
        let results = skill_data(self.core.agent_home())?;
        *self.cache.lock().unwrap() = Some(results.clone());
        Ok(results)
    }

    fn set_data(&self, value: SkillData) -> SentraResult<AssetMutationResult> {
        let Some(home) = &value.home else {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::MissingHome,
                format!("Skill {:?} has no home path", value.name),
            ));
        };
        let manifest_path = writable_manifest_path(self.core.agent_home());
        let skills_dir = manifest_path
            .parent()
            .unwrap_or(self.core.agent_home())
            .join("skills");
        let result = set_skill_data(&skills_dir, home)?;
        upsert_manifest_skill(&manifest_path, &value)?;
        *self.cache.lock().unwrap() = None;
        Ok(result)
    }

    fn del_data(&self, item: &SkillData) -> SentraResult<AssetMutationResult> {
        let targets = matching_skills(self, &item.name)?;
        if targets.is_empty() {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotFound,
                format!("Skill {:?} not found", item.name),
            ));
        }
        let mut changed = false;
        let mut errors = Vec::new();
        for skill in targets {
            let Some(home) = &skill.home else {
                continue;
            };
            if !path_inside(self.core.agent_home(), home) {
                errors.push(crate::interfaces::AssetMutationError {
                    code: AssetMutationErrorCode::PathOutsideAgentHome,
                    message: format!("Skill path is outside Claude App home: {}", home.display()),
                });
                continue;
            }
            let manifest_path = home
                .parent()
                .and_then(|path| path.parent())
                .map(|path| path.join("manifest.json"));
            let result = del_skill_data(home)?;
            changed |= result.changed;
            errors.extend(result.errors);
            if let Some(manifest_path) = manifest_path
                && manifest_path.exists()
            {
                remove_manifest_skill(&manifest_path, &skill)?;
            }
        }
        if changed {
            *self.cache.lock().unwrap() = None;
        }
        Ok(AssetMutationResult { changed, errors })
    }
}

fn matching_skills(asset: &SkillAsset, skill_name: &str) -> SentraResult<Vec<SkillData>> {
    let cached = asset.cache.lock().unwrap().clone();
    let skills = match cached {
        Some(skills) => skills,
        None => asset.get_data()?,
    };
    Ok(skills
        .into_iter()
        .filter(|skill| skill.name == skill_name)
        .collect())
}

fn skill_data(agent_home: &std::path::Path) -> SentraResult<Vec<SkillData>> {
    let base_path = skills_plugin_path(agent_home);
    let mut results = Vec::new();
    for manifest_path in find_manifest_files(&base_path, 0) {
        let manifest = read_manifest(&manifest_path)?;
        let skills_dir = manifest_path.parent().unwrap_or(&base_path).join("skills");
        for mut skill in collect_skills_from_dir(skills_dir)? {
            if let Some(entry) = manifest_entry_for(&manifest, &skill) {
                skill.enabled = entry
                    .get("enabled")
                    .and_then(|value| value.as_bool())
                    .or(skill.enabled);
                skill.source = entry
                    .get("creatorType")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .or(skill.source);
                skill.author = skill.author.or_else(|| {
                    entry
                        .get("creatorType")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                });
                skill.updated_at = entry
                    .get("updatedAt")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .or(skill.updated_at);
            }
            results.push(skill);
        }
    }
    Ok(results)
}

fn skills_plugin_path(agent_home: &std::path::Path) -> std::path::PathBuf {
    agent_home
        .join("local-agent-mode-sessions")
        .join("skills-plugin")
}

fn find_manifest_files(dir: &std::path::Path, depth: usize) -> Vec<std::path::PathBuf> {
    if depth > 3 || !dir_exists(dir) {
        return Vec::new();
    }
    let mut results = Vec::new();
    let manifest_path = dir.join("manifest.json");
    if read_json_file(&manifest_path).ok().flatten().is_some() {
        results.push(manifest_path);
    }
    for entry in std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if is_directory(&path) {
            results.extend(find_manifest_files(&path, depth + 1));
        }
    }
    results
}

fn read_manifest(
    manifest_path: &std::path::Path,
) -> SentraResult<std::collections::HashMap<String, serde_json::Value>> {
    let mut result = std::collections::HashMap::new();
    let Some(data) = read_json_file(manifest_path)? else {
        return Ok(result);
    };
    if let Some(skills) = data.get("skills").and_then(|value| value.as_array()) {
        for entry in skills {
            if let Some(skill_id) = entry.get("skillId").and_then(|value| value.as_str()) {
                result.insert(skill_id.to_string(), entry.clone());
            }
        }
    }
    Ok(result)
}

fn manifest_entry_for<'a>(
    manifest: &'a std::collections::HashMap<String, serde_json::Value>,
    skill: &SkillData,
) -> Option<&'a serde_json::Value> {
    manifest
        .get(&skill.name)
        .or_else(|| skill.id.as_ref().and_then(|id| manifest.get(id)))
}

fn writable_manifest_path(agent_home: &std::path::Path) -> std::path::PathBuf {
    let base = skills_plugin_path(agent_home);
    find_manifest_files(&base, 0)
        .into_iter()
        .next()
        .unwrap_or_else(|| base.join("sentra").join("manifest.json"))
}

fn upsert_manifest_skill(manifest_path: &std::path::Path, skill: &SkillData) -> SentraResult<()> {
    let mut manifest = read_json_file(manifest_path)?.unwrap_or_else(|| serde_json::json!({}));
    if !manifest.is_object() {
        manifest = serde_json::json!({});
    }
    if !manifest.get("skills").is_some_and(|value| value.is_array()) {
        manifest["skills"] = serde_json::json!([]);
    }
    let now = chrono::Utc::now().to_rfc3339();
    let skills = manifest
        .get_mut("skills")
        .and_then(|value| value.as_array_mut())
        .unwrap();
    let creator_type = skill.source.as_ref().or(skill.author.as_ref()).cloned();
    if let Some(entry) = skills
        .iter_mut()
        .find(|entry| entry.get("skillId").and_then(|value| value.as_str()) == Some(&skill.name))
    {
        entry["name"] = serde_json::json!(skill.name);
        entry["description"] = serde_json::json!(skill.description);
        if let Some(creator_type) = creator_type {
            entry["creatorType"] = serde_json::json!(creator_type);
        }
        entry["enabled"] = serde_json::json!(skill.enabled.unwrap_or(true));
        entry["updatedAt"] = serde_json::json!(now);
    } else {
        let mut entry = serde_json::json!({
            "skillId": skill.name,
            "name": skill.name,
            "description": skill.description,
            "enabled": skill.enabled.unwrap_or(true),
            "updatedAt": now,
        });
        if let Some(creator_type) = creator_type {
            entry["creatorType"] = serde_json::json!(creator_type);
        }
        skills.push(entry);
    }
    manifest["lastUpdated"] = serde_json::json!(chrono::Utc::now().timestamp_millis());
    backup_file(manifest_path)?;
    write_json_file(manifest_path, &manifest)
}

fn remove_manifest_skill(manifest_path: &std::path::Path, skill: &SkillData) -> SentraResult<()> {
    let Some(mut manifest) = read_json_file(manifest_path)? else {
        return Ok(());
    };
    let mut ids = std::collections::HashSet::from([skill.name.clone()]);
    if let Some(id) = &skill.id {
        ids.insert(id.clone());
    }
    if let Some(home_name) = skill
        .home
        .as_ref()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().to_string())
    {
        ids.insert(home_name);
    }
    if let Some(skills) = manifest
        .get_mut("skills")
        .and_then(|value| value.as_array_mut())
    {
        skills.retain(|entry| {
            entry
                .get("skillId")
                .and_then(|value| value.as_str())
                .is_none_or(|skill_id| !ids.contains(skill_id))
        });
    }
    manifest["lastUpdated"] = serde_json::json!(chrono::Utc::now().timestamp_millis());
    backup_file(manifest_path)?;
    write_json_file(manifest_path, &manifest)
}

fn path_inside(root: &std::path::Path, path: &std::path::Path) -> bool {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use crate::interfaces::{Asset, SkillData};

    use super::SkillAsset;

    #[test]
    fn set_data_invalidates_cached_skill_listing() {
        let agent_home = tempfile::tempdir().unwrap();
        let source_root = tempfile::tempdir().unwrap();
        let source_skill = source_root.path().join("new-skill");
        std::fs::create_dir_all(&source_skill).unwrap();
        std::fs::write(
            source_skill.join("SKILL.md"),
            "---\nname: new-skill\n---\nbody",
        )
        .unwrap();

        let asset = SkillAsset::new("claude-app", agent_home.path());
        assert!(asset.get_data().unwrap().is_empty());

        asset
            .set_data(SkillData {
                name: "new-skill".to_string(),
                home: Some(source_skill),
                ..SkillData::default()
            })
            .unwrap();

        assert!(
            asset
                .get_data()
                .unwrap()
                .iter()
                .any(|skill| skill.name == "new-skill")
        );
    }
}
