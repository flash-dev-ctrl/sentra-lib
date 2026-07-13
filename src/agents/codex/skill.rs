use std::sync::{Arc, Mutex};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, SkillData};
use crate::utils::{
    collect_skills_from_dir, del_skill_data, dir_exists, is_directory, read_json_file,
    read_text_file, set_skill_data,
};

#[derive(Debug, Clone)]
pub(super) struct SkillAsset {
    pub(crate) core: AssetCore,
    cache: Arc<Mutex<Option<Vec<SkillData>>>>,
}

impl SkillAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        let agent_name = agent_name.into();
        let agent_home = agent_home.into();
        Self {
            core: AssetCore::new(agent_name, agent_home),
            cache: Arc::new(Mutex::new(None)),
        }
    }
}

impl_erased_asset!(SkillAsset, AssetType::Skill, Vec<SkillData>, SkillData);

impl Asset<Vec<SkillData>, SkillData> for SkillAsset {
    fn get_data(&self) -> SentraResult<Vec<SkillData>> {
        if let Some(cached) = self.cache.lock().unwrap().clone() {
            return Ok(cached);
        }
        let home = self.core.agent_home();
        let enabled_map = parse_enabled_map(&home.join("config.toml"))?;
        let mut results = collect_skills_with_enabled(&home.join("skills"), &enabled_map)?;
        let plugin_cache_dir = home.join("plugins").join("cache");
        if !dir_exists(&plugin_cache_dir) {
            *self.cache.lock().unwrap() = Some(results.clone());
            return Ok(results);
        }
        for vendor in read_dir_paths(&plugin_cache_dir) {
            if !is_directory(&vendor) {
                continue;
            }
            for channel in read_dir_paths(&vendor) {
                if !is_directory(&channel) {
                    continue;
                }
                for plugin_json_path in find_plugin_json_files(&channel, 4, 0) {
                    let Some(plugin_raw) = read_json_file(&plugin_json_path)? else {
                        continue;
                    };
                    let Some(skills_rel) =
                        plugin_raw.get("skills").and_then(|value| value.as_str())
                    else {
                        continue;
                    };
                    let Some(plugin_root) =
                        plugin_json_path.parent().and_then(|path| path.parent())
                    else {
                        continue;
                    };
                    let skills_dir = plugin_root.join(skills_rel);
                    if !dir_exists(&skills_dir) {
                        continue;
                    }
                    let author_name = plugin_raw.get("author").and_then(|author| {
                        author.as_str().map(str::to_string).or_else(|| {
                            author
                                .get("name")
                                .and_then(|value| value.as_str())
                                .map(str::to_string)
                        })
                    });
                    for mut skill in collect_skills_with_enabled(&skills_dir, &enabled_map)? {
                        skill.source = plugin_raw
                            .get("name")
                            .and_then(|value| value.as_str())
                            .map(str::to_string)
                            .or(skill.source);
                        skill.author = skill.author.or_else(|| author_name.clone());
                        skill.version = skill.version.or_else(|| {
                            plugin_raw
                                .get("version")
                                .and_then(|value| value.as_str())
                                .map(str::to_string)
                        });
                        results.push(skill);
                    }
                }
            }
        }
        *self.cache.lock().unwrap() = Some(results.clone());
        Ok(results)
    }

    fn set_data(&self, value: SkillData) -> SentraResult<AssetMutationResult> {
        let Some(home) = &value.home else {
            return Ok(crate::interfaces::AssetMutationResult::unchanged(
                crate::interfaces::AssetMutationErrorCode::MissingHome,
                format!("Skill {:?} has no home path", value.name),
            ));
        };
        set_skill_data(&self.core.agent_home().join("skills"), home)
    }

    fn del_data(&self, item: &SkillData) -> SentraResult<AssetMutationResult> {
        let skill_name = &item.name;
        let cached = {
            let guard = self.cache.lock().unwrap();
            guard.clone()
        };
        let skills = match cached {
            Some(v) => v,
            None => self.get_data()?,
        };
        let targets: Vec<_> = skills
            .into_iter()
            .filter(|s| &s.name == skill_name)
            .filter_map(|s| s.home)
            .collect();
        if targets.is_empty() {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotFound,
                format!("Skill {:?} not found", skill_name),
            ));
        }
        let mut changed = false;
        let mut errors = Vec::new();
        for path in targets {
            let result = del_skill_data(&path)?;
            changed |= result.changed;
            errors.extend(result.errors);
        }
        if changed {
            *self.cache.lock().unwrap() = None;
        }
        Ok(AssetMutationResult { changed, errors })
    }
}

fn parse_enabled_map(
    config_path: &std::path::Path,
) -> SentraResult<std::collections::HashMap<String, bool>> {
    let Some(content) = read_text_file(config_path)? else {
        return Ok(std::collections::HashMap::new());
    };
    let Ok(parsed) = toml::from_str::<toml::Value>(&content) else {
        return Ok(std::collections::HashMap::new());
    };
    let mut map = std::collections::HashMap::new();
    let configs = parsed
        .get("skills")
        .and_then(|value| value.get("config"))
        .and_then(|value| value.as_array());
    if let Some(configs) = configs {
        for entry in configs {
            let Some(path) = entry.get("path").and_then(|value| value.as_str()) else {
                continue;
            };
            let enabled = entry
                .get("enabled")
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
            map.insert(normalize_config_path(path), enabled);
        }
    }
    Ok(map)
}

fn collect_skills_with_enabled(
    dir: &std::path::Path,
    enabled_map: &std::collections::HashMap<String, bool>,
) -> SentraResult<Vec<SkillData>> {
    let mut skills = collect_skills_from_dir(dir)?;
    for skill in &mut skills {
        if let Some(home) = &skill.home {
            let key = normalize_string_path(home.join("SKILL.md").to_string_lossy().as_ref());
            if let Some(enabled) = enabled_map.get(&key) {
                skill.enabled = Some(*enabled);
            }
        }
    }
    Ok(skills)
}

fn find_plugin_json_files(
    dir: &std::path::Path,
    max_depth: usize,
    depth: usize,
) -> Vec<std::path::PathBuf> {
    if depth > max_depth || !dir_exists(dir) {
        return Vec::new();
    }
    let candidate = dir.join("plugin.json");
    if candidate.is_file() {
        return vec![candidate];
    }
    let mut results = Vec::new();
    for entry in read_dir_paths(dir) {
        if is_directory(&entry) {
            results.extend(find_plugin_json_files(&entry, max_depth, depth + 1));
        }
    }
    results
}

fn read_dir_paths(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
}

fn normalize_string_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn normalize_config_path(path: &str) -> String {
    let path = std::path::Path::new(path)
        .canonicalize()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    clean_windows_verbatim_prefix(&path).replace('\\', "/")
}

fn clean_windows_verbatim_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    path.to_string()
}
