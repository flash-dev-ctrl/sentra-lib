use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{Asset, AssetType, SkillData};
use crate::utils::{collect_skills_from_dir, read_json_file};
use crate::SentraResult;

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
        let mut results = collect_skills_from_dir(self.core.agent_home().join("skills"))?;
        for manifest_path in super::plugin_manifests(self.core.agent_home()) {
            let Some(manifest) = read_json_file(&manifest_path)? else {
                continue;
            };
            let Some(skills_dir) = manifest
                .get("skills")
                .and_then(|value| value.as_str())
                .and_then(|rel| manifest_path.parent().map(|dir| dir.join(rel)))
            else {
                continue;
            };
            let source = manifest.get("name").and_then(|value| value.as_str());
            for mut skill in collect_skills_from_dir(skills_dir)? {
                skill.source = source.map(str::to_string).or(skill.source);
                results.push(skill);
            }
        }
        Ok(results)
    }
}
