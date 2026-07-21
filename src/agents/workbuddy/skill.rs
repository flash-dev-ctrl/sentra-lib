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
        collect_skills_from_dir(self.core.agent_home().join("skills"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_workbuddy_skills_directory() {
        let dir = tempfile::tempdir().unwrap();
        let skill = dir.path().join("skills").join("demo");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(
            skill.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo\n---\n",
        )
        .unwrap();

        let data = <SkillAsset as Asset<Vec<SkillData>>>::get_data(&SkillAsset::new(
            "workbuddy",
            dir.path(),
        ))
        .unwrap();

        assert_eq!(data.len(), 1);
        assert_eq!(data[0].name, "demo");
    }
}
