use std::sync::{Arc, Mutex};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, SkillData};
use crate::utils::{collect_skills_from_dir, del_skill_data, set_skill_data};

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
        let agent_name = agent_name.into();
        let agent_home = agent_home.into();
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
        let data = collect_skills_from_dir(self.core.agent_home().join("skills"))?;
        *self.cache.lock().unwrap() = Some(data.clone());
        Ok(data)
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
        for path in targets {
            let result = del_skill_data(&path)?;
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

#[cfg(test)]
mod tests {
    use crate::interfaces::{Asset, SkillData};

    use super::SkillAsset;

    #[test]
    fn del_data_deletes_all_cached_skills_with_matching_name() {
        let agent_home = tempfile::tempdir().unwrap();
        for path in [
            agent_home.path().join("skills").join("dup-a"),
            agent_home
                .path()
                .join("skills")
                .join("nested")
                .join("dup-b"),
        ] {
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("SKILL.md"), "---\nname: duplicate\n---\nbody").unwrap();
        }

        let asset = SkillAsset::new("sentra", agent_home.path());
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
