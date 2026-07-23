use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, SkillData};
use crate::utils::collect_skills_from_dir;

#[derive(Debug, Clone)]
pub(super) struct SkillAsset {
    core: AssetCore,
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
        let mut results = collect_skills_from_dir(
            crate::agents::kimi::app_daimon_home(self.core.agent_home()).join("skills"),
        )?;
        results.extend(crate::agents::kimi::skill::plugin_skill_data(
            &crate::agents::kimi::app_runtime_home(self.core.agent_home()),
        )?);
        Ok(crate::agents::kimi::skill::dedup_skills(results))
    }
}
