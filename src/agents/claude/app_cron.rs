use std::collections::HashSet;

use serde::Deserialize;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, CronData, CronType};
use crate::utils::{collect_skill_files, dir_exists, read_text_file};

#[derive(Debug, Clone)]
pub(super) struct CronAsset {
    pub(crate) core: AssetCore,
}

impl CronAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(CronAsset, AssetType::Cron, Vec<CronData>);

impl Asset<Vec<CronData>> for CronAsset {
    fn get_data(&self) -> SentraResult<Vec<CronData>> {
        cron_data(self.core.agent_home())
    }
}

fn cron_data(agent_home: &std::path::Path) -> SentraResult<Vec<CronData>> {
    let sessions_dir = agent_home.join("claude-code-sessions");
    let mut task_files = Vec::new();
    find_task_files(&sessions_dir, &mut task_files, 0);
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for task_file in task_files {
        let Some(raw) = read_text_file(&task_file)? else {
            continue;
        };
        let Ok(config) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let Some(tasks) = config
            .get("scheduledTasks")
            .and_then(|value| value.as_array())
        else {
            continue;
        };
        for task in tasks {
            let Some(id) = task.get("id").and_then(|value| value.as_str()) else {
                continue;
            };
            if !seen.insert(id.to_string()) {
                continue;
            }
            let file_path = task
                .get("filePath")
                .and_then(|value| value.as_str())
                .map(std::path::PathBuf::from);
            let skill_home = file_path
                .as_ref()
                .and_then(|path| path.parent())
                .map(std::path::Path::to_path_buf);
            let files = skill_home
                .as_ref()
                .map(|home| collect_skill_files(home, 12))
                .transpose()?
                .unwrap_or_default();
            let frontmatter = file_path
                .as_ref()
                .and_then(|path| read_text_file(path).ok().flatten())
                .and_then(|content| parse_frontmatter(&content).ok());
            let schedule = task
                .get("cronExpression")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            results.push(CronData {
                id: id.to_string(),
                name: frontmatter
                    .as_ref()
                    .and_then(|value| value.name.clone())
                    .unwrap_or_else(|| id.to_string()),
                prompt: frontmatter
                    .and_then(|value| value.description)
                    .unwrap_or_default(),
                enabled: task
                    .get("enabled")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
                home: skill_home,
                cron_type: schedule.as_ref().map(|_| CronType::Cron),
                schedule,
                cwds: task
                    .get("cwd")
                    .and_then(|value| value.as_str())
                    .map(|cwd| vec![cwd.to_string()])
                    .unwrap_or_default(),
                created_at: task.get("createdAt").and_then(|value| value.as_f64()),
                updated_at: task.get("updatedAt").and_then(|value| value.as_f64()),
                files,
            });
        }
    }
    Ok(results)
}

fn find_task_files(dir: &std::path::Path, results: &mut Vec<std::path::PathBuf>, depth: usize) {
    if depth > 6 || !dir_exists(dir) {
        return;
    }
    for entry in std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            find_task_files(&path, results, depth + 1);
        } else if entry.file_name() == "scheduled-tasks.json" {
            results.push(path);
        }
    }
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
}

fn parse_frontmatter(content: &str) -> SentraResult<SkillFrontmatter> {
    let Some(rest) = content.strip_prefix("---") else {
        return Ok(SkillFrontmatter {
            name: None,
            description: None,
        });
    };
    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .unwrap_or(rest);
    let Some((frontmatter, _body)) = rest.split_once("\n---") else {
        return Ok(SkillFrontmatter {
            name: None,
            description: None,
        });
    };
    Ok(
        serde_yaml::from_str(frontmatter).unwrap_or(SkillFrontmatter {
            name: None,
            description: None,
        }),
    )
}
