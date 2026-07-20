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
        let mut out = collect_skills_from_dir(self.core.agent_home().join("skills"))?;
        out.extend(collect_skills_from_dir(self.core.agent_home().join("agents"))?);
        let cwd = std::env::current_dir().unwrap_or_default();
        let project_home = cwd.join(format!(".{}", self.core.agent_name()));
        out.extend(collect_skills_from_dir(project_home.join("skills"))?);
        out.extend(collect_skills_from_dir(project_home.join("agents"))?);
        Ok(out)
    }
}
