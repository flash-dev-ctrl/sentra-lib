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
    let mut results = collect_skills_from_dir(state_home.join("skills"))?;
    results.extend(collect_skills_from_dir(state_home.join("builtin_skills"))?);
    results.extend(collect_builtin_surface_skills(&state_home, "trae")?);
    results.extend(collect_builtin_surface_skills(&state_home, "code")?);
    if let Some(path) = crate::agents::trae::workspace_path(".trae/skills") {
        results.extend(collect_skills_from_dir(path)?);
    }
    let user_home = surface::user_home(agent_name, home);
    if let Some(path) = crate::agents::workspace_agents_dir(&user_home) {
        results.extend(collect_skills_from_dir(path.join("skills"))?);
    }
    apply_skill_config(&state_home, &mut results)?;
    Ok(dedup_skills(results))
}

fn collect_builtin_surface_skills(
    home: &std::path::Path,
    surface_dir: &str,
) -> SentraResult<Vec<SkillData>> {
    let mut out = Vec::new();
    let root = home.join("builtin").join(surface_dir);
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
    let builtin_status = config
        .get("builtinSkillStatus")
        .and_then(|value| value.as_object());
    let managed = config
        .get("managedSkills")
        .and_then(|value| value.as_object());

    for skill in skills {
        if disabled.contains(skill.name.as_str()) {
            skill.enabled = Some(false);
        }
        if let Some(enabled) = builtin_status
            .and_then(|items| items.get(&skill.name))
            .and_then(|value| value.as_bool())
        {
            skill.enabled = Some(enabled);
        }
        if let Some(source) = managed
            .and_then(|items| items.get(&skill.name))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            skill.source = Some(source.to_string());
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
    fn applies_trae_skill_config_status_and_source() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".trae");
        write_skill(&home, "skills", "market-skill");
        write_skill(&home, "skills", "disabled-skill");
        std::fs::write(
            home.join("skill-config.json"),
            r#"{
              "managedSkills": {"market-skill": "marketplace"},
              "disabledSkills": ["disabled-skill"],
              "builtinSkillStatus": {"market-skill": true}
            }"#,
        )
        .unwrap();

        let skills =
            <SkillAsset as Asset<Vec<SkillData>>>::get_data(&SkillAsset::new("trae-ide", &home))
                .unwrap();

        let market = skills
            .iter()
            .find(|skill| skill.name == "market-skill")
            .unwrap();
        let disabled = skills
            .iter()
            .find(|skill| skill.name == "disabled-skill")
            .unwrap();
        assert_eq!(market.enabled, Some(true));
        assert_eq!(market.source.as_deref(), Some("marketplace"));
        assert_eq!(disabled.enabled, Some(false));
    }

    #[test]
    fn reads_builtin_ide_skills() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".trae");
        write_skill(
            &home,
            std::path::Path::new("builtin")
                .join("trae")
                .join("default")
                .join("skills"),
            "ide-demo",
        );

        let skills = skill_data("trae-ide", &home).unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "ide-demo");
    }

    fn write_skill(home: &std::path::Path, base: impl AsRef<std::path::Path>, name: &str) {
        let skill = home.join(base).join(name);
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), format!("---\nname: {name}\n---\n")).unwrap();
    }
}
