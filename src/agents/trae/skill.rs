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

impl_erased_asset!(SkillAsset, AssetType::Skill, Vec<SkillData>);

impl Asset<Vec<SkillData>> for SkillAsset {
    fn get_data(&self) -> SentraResult<Vec<SkillData>> {
        let home = self.core.agent_home();
        let mut results = collect_skills_from_dir(home.join("skills"))?;
        if let Some(path) = crate::agents::trae::workspace_path(".trae/skills") {
            results.extend(collect_skills_from_dir(path)?);
        }
        let user_home = home.parent().unwrap_or(home);
        if let Some(path) = crate::agents::workspace_agents_dir(user_home) {
            results.extend(collect_skills_from_dir(path.join("skills"))?);
        }
        Ok(results)
    }
}
