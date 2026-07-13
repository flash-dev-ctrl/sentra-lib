use std::sync::{Arc, Mutex};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, SkillData};
use crate::utils::{collect_skills_from_dir, del_skill_data, is_path_inside, set_skill_data};

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
        let skills = skill_data(self.core.agent_home())?;
        *self.cache.lock().unwrap() = Some(skills.clone());
        Ok(skills)
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
            if !is_path_inside(self.core.agent_home(), &home) {
                errors.push(crate::interfaces::AssetMutationError {
                    code: AssetMutationErrorCode::PathOutsideAgentHome,
                    message: format!("Skill path is outside agent home: {}", home.display()),
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
    results.extend(collect_skills_from_dir(
        agent_home.join("hermes-agent").join("skills"),
    )?);
    Ok(results)
}
