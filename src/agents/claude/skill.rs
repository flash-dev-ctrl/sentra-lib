use std::sync::{Arc, Mutex};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, SkillData};
use crate::utils::{
    collect_skills_from_dir, del_skill_data, dir_exists, is_directory, read_json_file,
    set_skill_data,
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
        let result = set_skill_data(&self.core.agent_home().join("skills"), home)?;
        if result.changed {
            *self.cache.lock().unwrap() = None;
        }
        Ok(result)
    }

    fn del_data(&self, item: &SkillData) -> SentraResult<AssetMutationResult> {
        let targets = matching_skill_homes(self, &item.name)?;
        if targets.is_empty() {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotFound,
                format!("Skill {:?} not found", item.name),
            ));
        }
        let mut changed = false;
        let mut errors = Vec::new();
        for home in targets {
            if !path_inside(self.core.agent_home(), &home) {
                errors.push(crate::interfaces::AssetMutationError {
                    code: AssetMutationErrorCode::PathOutsideAgentHome,
                    message: format!("Skill path is outside Claude Code home: {}", home.display()),
                });
                continue;
            }
            let result = del_skill_data(&home)?;
            changed |= result.changed;
            errors.extend(result.errors);
        }
        if changed {
            *self.cache.lock().unwrap() = None;
        }
        Ok(AssetMutationResult { changed, errors })
    }
}

fn matching_skill_homes(
    asset: &SkillAsset,
    skill_name: &str,
) -> SentraResult<Vec<std::path::PathBuf>> {
    let cached = asset.cache.lock().unwrap().clone();
    let skills = match cached {
        Some(skills) => skills,
        None => asset.get_data()?,
    };
    Ok(skills
        .into_iter()
        .filter(|skill| skill.name == skill_name)
        .filter_map(|skill| skill.home)
        .collect())
}

fn skill_data(agent_home: &std::path::Path) -> SentraResult<Vec<SkillData>> {
    let mut results = collect_skills_from_dir(agent_home.join("skills"))?;
    let plugin_cache_dir = agent_home.join("plugins").join("cache");
    if !dir_exists(&plugin_cache_dir) {
        return Ok(results);
    }
    for marketplace_dir in read_dir_paths(&plugin_cache_dir) {
        if !is_directory(&marketplace_dir) {
            continue;
        }
        for plugin_dir in read_dir_paths(&marketplace_dir) {
            if !is_directory(&plugin_dir) {
                continue;
            }
            for version_dir in read_dir_paths(&plugin_dir) {
                if !is_directory(&version_dir) {
                    continue;
                }
                let skills_dir = version_dir.join("skills");
                if !dir_exists(&skills_dir) {
                    continue;
                }
                let plugin =
                    read_json_file(version_dir.join(".claude-plugin").join("plugin.json"))?;
                let author = plugin.as_ref().and_then(author_name);
                let source = plugin
                    .as_ref()
                    .and_then(|value| value.get("name"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .or_else(|| {
                        plugin_dir
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                    });
                let version = plugin
                    .as_ref()
                    .and_then(|value| value.get("version"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .or_else(|| {
                        version_dir
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                    });
                for mut skill in collect_skills_from_dir(&skills_dir)? {
                    skill.source = source.clone();
                    skill.author = skill.author.or_else(|| author.clone());
                    skill.version = skill.version.or_else(|| version.clone());
                    results.push(skill);
                }
            }
        }
    }
    Ok(results)
}

fn author_name(plugin: &serde_json::Value) -> Option<String> {
    plugin.get("author").and_then(|author| {
        author.as_str().map(str::to_string).or_else(|| {
            author
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
    })
}

fn read_dir_paths(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
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

        let asset = SkillAsset::new("claude-cli", agent_home.path());
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

    #[test]
    fn del_data_deletes_all_cached_skills_with_matching_name() {
        let agent_home = tempfile::tempdir().unwrap();
        for path in [
            agent_home.path().join("skills").join("dup-a"),
            agent_home
                .path()
                .join("plugins")
                .join("cache")
                .join("market")
                .join("plugin")
                .join("1.0.0")
                .join("skills")
                .join("dup-b"),
        ] {
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("SKILL.md"), "---\nname: duplicate\n---\nbody").unwrap();
        }

        let asset = SkillAsset::new("claude-cli", agent_home.path());
        assert_eq!(asset.get_data().unwrap().len(), 2);

        let result = asset
            .del_data(&SkillData {
                name: "duplicate".to_string(),
                ..SkillData::default()
            })
            .unwrap();

        assert!(result.changed);
        assert!(asset.get_data().unwrap().is_empty());
    }
}
