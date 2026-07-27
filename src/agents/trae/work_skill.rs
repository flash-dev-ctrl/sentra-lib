use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::agents::trae::surface;
use crate::interfaces::{Asset, AssetType, SkillData};
use crate::utils::{collect_skills_from_dir, read_json_file};

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
        skill_data(self.core.agent_name(), self.core.agent_home())
    }
}

fn skill_data(agent_name: &str, home: &std::path::Path) -> SentraResult<Vec<SkillData>> {
    let state_home = surface::state_home(agent_name, home);
    let work_home = state_home.join("work");
    let mut results = collect_skills_from_dir(home.join("skills"))?;
    results.extend(collect_skills_from_dir(work_home.join("skills"))?);
    results.extend(collect_builtin_work_skills(&state_home)?);
    apply_skill_config(&work_home, &mut results)?;
    Ok(dedup_skills(results))
}

fn collect_builtin_work_skills(state_home: &std::path::Path) -> SentraResult<Vec<SkillData>> {
    let mut out = Vec::new();
    let root = state_home.join("builtin").join("work");
    for entry in std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
    {
        out.extend(collect_skills_from_dir(entry.path().join("skills"))?);
    }
    Ok(out)
}

fn apply_skill_config(home: &std::path::Path, skills: &mut [SkillData]) -> SentraResult<()> {
    let Some(config) = read_json_file(home.join("skill-config.json"))? else {
        return Ok(());
    };
    let disabled = config
        .get("disabledSkills")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .collect::<std::collections::HashSet<_>>();
    for skill in skills {
        if disabled.contains(skill.name.as_str()) {
            skill.enabled = Some(false);
        }
    }
    Ok(())
}

fn dedup_skills(skills: Vec<SkillData>) -> Vec<SkillData> {
    let mut out = Vec::new();
    for skill in skills {
        let home = skill.home.clone();
        if out
            .iter()
            .any(|seen: &SkillData| seen.name == skill.name && seen.home == home)
        {
            continue;
        }
        out.push(skill);
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::interfaces::{Asset, SkillData};

    use super::*;

    #[test]
    fn reads_work_builtin_skills_without_ide_workspace_skills() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".trae").join("work");
        let state_home = dir.path().join(".trae");
        write_skill(
            &state_home,
            std::path::Path::new("builtin")
                .join("work")
                .join("default")
                .join("skills"),
            "work-demo",
        );
        write_skill(&state_home, "skills", "ide-skill");

        let skills =
            <SkillAsset as Asset<Vec<SkillData>>>::get_data(&SkillAsset::new("trae-work", &home))
                .unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "work-demo");
    }

    fn write_skill(home: &std::path::Path, base: impl AsRef<std::path::Path>, name: &str) {
        let skill = home.join(base).join(name);
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), format!("---\nname: {name}\n---\n")).unwrap();
    }
}
