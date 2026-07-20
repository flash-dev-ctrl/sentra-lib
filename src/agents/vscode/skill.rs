use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, SkillData};
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

impl_erased_asset!(SkillAsset, AssetType::Skill, Vec<SkillData>);

impl Asset<Vec<SkillData>> for SkillAsset {
    fn get_data(&self) -> SentraResult<Vec<SkillData>> {
        let home = super::user_home(self.core.agent_home());
        let cwd = std::env::current_dir().unwrap_or_default();
        let mut results = Vec::new();
        for dir in [
            home.join(".copilot").join("skills"),
            home.join(".agents").join("skills"),
            cwd.join(".agents").join("skills"),
            cwd.join(".github").join("skills"),
            cwd.join(".agents").join("hooks"),
            cwd.join(".github").join("hooks"),
        ] {
            results.extend(collect_skills_from_dir(dir)?);
        }
        Ok(results)
    }
}
