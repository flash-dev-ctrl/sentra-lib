use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, CronData, CronType};

#[derive(Debug, Clone)]
pub(super) struct CronAsset {
    core: AssetCore,
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
        Ok(cron_data(self.core.agent_home()))
    }
}

fn cron_data(agent_home: &Path) -> Vec<CronData> {
    let mut cron_dirs = Vec::new();
    find_cron_dirs(&agent_home.join("sessions"), 0, &mut cron_dirs);
    cron_dirs.sort();

    let mut results = Vec::new();
    for cron_dir in cron_dirs {
        let cwd = session_work_dir(&cron_dir);
        let mut task_paths = read_dir_paths(&cron_dir)
            .into_iter()
            .filter(|path| {
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            })
            .collect::<Vec<_>>();
        task_paths.sort();
        for task_path in task_paths {
            let Ok(content) = std::fs::read_to_string(&task_path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&content) else {
                continue;
            };
            let Some(task) = cron_from_value(&value, &task_path, cwd.as_deref()) else {
                continue;
            };
            results.push(task);
        }
    }
    results
}

fn find_cron_dirs(dir: &Path, depth: usize, results: &mut Vec<PathBuf>) {
    if depth > 5 || !dir.is_dir() {
        return;
    }
    if dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("cron"))
    {
        results.push(dir.to_path_buf());
        return;
    }
    for path in read_dir_paths(dir) {
        if path.is_dir() {
            find_cron_dirs(&path, depth + 1, results);
        }
    }
}

fn cron_from_value(value: &Value, task_path: &Path, cwd: Option<&str>) -> Option<CronData> {
    let id = value.get("id")?.as_str()?;
    if !valid_cron_id(id)
        || task_path.file_stem().and_then(|stem| stem.to_str()) != Some(id)
        || !value.get("cron").is_some_and(Value::is_string)
        || !value.get("prompt").is_some_and(Value::is_string)
        || !value.get("createdAt").is_some_and(Value::is_number)
        || value
            .get("recurring")
            .is_some_and(|recurring| !recurring.is_boolean())
    {
        return None;
    }
    let schedule = value.get("cron")?.as_str()?.to_string();
    let prompt = value.get("prompt")?.as_str()?.to_string();
    Some(CronData {
        id: id.to_string(),
        name: id.to_string(),
        prompt,
        enabled: true,
        home: Some(task_path.to_path_buf()),
        cron_type: Some(CronType::Cron),
        schedule: Some(schedule),
        cwds: cwd.into_iter().map(str::to_string).collect(),
        created_at: value.get("createdAt").and_then(Value::as_f64),
        updated_at: value.get("lastFiredAt").and_then(Value::as_f64),
        files: Vec::new(),
    })
}

fn valid_cron_id(id: &str) -> bool {
    id.len() == 8
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn session_work_dir(cron_dir: &Path) -> Option<String> {
    let session_dir = cron_dir.parent()?;
    let content = std::fs::read_to_string(session_dir.join("state.json")).ok()?;
    serde_json::from_str::<Value>(&content)
        .ok()?
        .get("workDir")?
        .as_str()
        .map(str::to_string)
}

fn read_dir_paths(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_valid_session_cron_tasks_and_ignores_strays() {
        let dir = tempfile::tempdir().unwrap();
        let session = dir
            .path()
            .join("sessions")
            .join("wd_123")
            .join("session_123");
        let cron = session.join("cron");
        std::fs::create_dir_all(&cron).unwrap();
        std::fs::write(
            session.join("state.json"),
            r#"{"workDir":"C:/workspace/project","lastPrompt":"private"}"#,
        )
        .unwrap();
        std::fs::write(
            cron.join("1a2b3c4d.json"),
            r#"{"id":"1a2b3c4d","cron":"0 9 * * *","prompt":"summarize","createdAt":1000,"recurring":false,"lastFiredAt":2000}"#,
        )
        .unwrap();
        std::fs::write(cron.join("not-a-task.json"), "not-json").unwrap();

        let tasks = cron_data(dir.path());

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "1a2b3c4d");
        assert_eq!(tasks[0].schedule.as_deref(), Some("0 9 * * *"));
        assert!(tasks[0].enabled);
        assert_eq!(tasks[0].cwds, ["C:/workspace/project"]);
        assert_eq!(tasks[0].created_at, Some(1000.0));
        assert_eq!(tasks[0].updated_at, Some(2000.0));
    }
}
