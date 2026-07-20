use crate::agents::install_status::hidden_home_parent;
use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{Asset, AssetType, SkillData};
use crate::utils::collect_skills_from_dir;
use crate::SentraResult;

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

impl_erased_asset!(SkillAsset, AssetType::Skill, Vec<SkillData>);

impl Asset<Vec<SkillData>> for SkillAsset {
    fn get_data(&self) -> SentraResult<Vec<SkillData>> {
        let home = self.core.agent_home();
        let user_home = hidden_home_parent(home);
        let mut results = collect_skills_from_dir(home.join("skills"))?;
        results.extend(collect_skills_from_dir(
            user_home.join(".agents").join("skills"),
        )?);
        Ok(results)
    }
}
