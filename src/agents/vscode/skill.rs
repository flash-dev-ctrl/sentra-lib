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
        let workspace_agents = crate::agents::workspace_agents_dir(home);
        let mut dirs = vec![home.join(".copilot").join("skills")];
        if let Some(workspace_agents) = &workspace_agents {
            dirs.push(workspace_agents.join("skills"));
        }
        dirs.push(cwd.join(".github").join("skills"));
        if let Some(workspace_agents) = &workspace_agents {
            dirs.push(workspace_agents.join("hooks"));
        }
        dirs.push(cwd.join(".github").join("hooks"));

        let mut results = Vec::new();
        for dir in dirs {
            results.extend(collect_skills_from_dir(dir)?);
        }
        Ok(results)
    }
}
