use std::path::Path;

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::agents::qoder::surface;
use crate::interfaces::{Asset, AssetType, CronData, CronType};
use crate::utils::{SqliteDatabase, read_json_file};

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
        cron_data(self.core.agent_name(), self.core.agent_home())
    }
}

fn cron_data(agent_name: &str, agent_home: &Path) -> SentraResult<Vec<CronData>> {
    let mut out = Vec::new();
    for file in [
        "cron.json",
        "tasks.json",
        "scheduled_tasks.json",
        "scheduled-tasks.json",
        "settings.json",
    ] {
        let path = agent_home.join(file);
        let Some(config) = read_json_file(&path)? else {
            continue;
        };
        collect_crons(&config, &path, &mut out);
    }
    for app_root in surface::work_data_roots(agent_name, agent_home) {
        extend_database_crons(&app_root, &mut out)?;
    }
    Ok(dedup_crons(out))
}

fn extend_database_crons(app_root: &Path, out: &mut Vec<CronData>) -> SentraResult<()> {
    let database_path = app_root.join("data").join("agents.db");
    let Some(database) = SqliteDatabase::open_read_only(&database_path)? else {
        return Ok(());
    };
    if !database.table_exists("scheduled_tasks")? {
        return Ok(());
    }
    let tasks = database.query_map(
        "SELECT id, name, description, enabled, schedule, payload, created_at, updated_at \
         FROM scheduled_tasks WHERE deleted_at IS NULL",
        rusqlite::params![],
        |row| {
            Ok(DatabaseTask {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                enabled: row.get::<_, Option<i64>>(3)?.map(|value| value != 0),
                schedule: row.get(4)?,
                payload: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        },
    )?;
    out.extend(
        tasks
            .into_iter()
            .filter_map(|task| cron_from_database_task(task, &database_path)),
    );
    Ok(())
}

struct DatabaseTask {
    id: String,
    name: String,
    description: Option<String>,
    enabled: Option<bool>,
    schedule: String,
    payload: String,
    created_at: Option<i64>,
    updated_at: Option<i64>,
}

fn cron_from_database_task(task: DatabaseTask, database_path: &Path) -> Option<CronData> {
    let schedule = serde_json::from_str::<Value>(&task.schedule)
        .unwrap_or_else(|_| Value::String(task.schedule.clone()));
    let wrapper = serde_json::json!({ "schedule": schedule });
    let (cron_type, schedule) = schedule_of(&wrapper);
    let payload = serde_json::from_str::<Value>(&task.payload).unwrap_or(Value::Null);
    Some(CronData {
        id: task.id,
        name: task.name,
        prompt: string_field(&payload, &["message", "prompt", "description"])
            .or(task.description)
            .unwrap_or_default(),
        enabled: task.enabled.unwrap_or(true),
        home: Some(database_path.to_path_buf()),
        cron_type,
        schedule,
        cwds: cwds(&payload),
        created_at: task.created_at.map(|value| value as f64),
        updated_at: task.updated_at.map(|value| value as f64),
        files: Vec::new(),
    })
}

fn collect_crons(value: &Value, path: &Path, out: &mut Vec<CronData>) {
    let data = value.get("data").unwrap_or(value);
    if let Some(items) = data.as_array() {
        out.extend(items.iter().filter_map(|item| cron_from_value(item, path)));
    }
    for key in ["tasks", "cron", "scheduledTasks", "scheduled_tasks"] {
        let Some(items) = data.get(key) else {
            continue;
        };
        if let Some(items) = items.as_array() {
            out.extend(items.iter().filter_map(|item| cron_from_value(item, path)));
        } else if let Some(items) = items.as_object() {
            out.extend(
                items
                    .values()
                    .filter_map(|item| cron_from_value(item, path)),
            );
        }
    }
    if data.get("schedule").is_some() && data.get("id").is_some() {
        if let Some(task) = cron_from_value(data, path) {
            out.push(task);
        }
    }
}

fn cron_from_value(value: &Value, path: &Path) -> Option<CronData> {
    let id = string_field(value, &["id", "taskId", "name"])?;
    let (cron_type, schedule) = schedule_of(value);
    Some(CronData {
        id: id.clone(),
        name: string_field(value, &["name", "title"]).unwrap_or(id),
        prompt: string_field(value, &["prompt", "description"]).unwrap_or_default(),
        enabled: value
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true)
            && !matches!(
                value.get("status").and_then(Value::as_str),
                Some("disabled" | "cancelled")
            ),
        home: Some(path.to_path_buf()),
        cron_type,
        schedule,
        cwds: cwds(value),
        created_at: timestamp(value.get("createdAt").or_else(|| value.get("created_at"))),
        updated_at: timestamp(value.get("updatedAt").or_else(|| value.get("updated_at"))),
        files: Vec::new(),
    })
}

fn schedule_of(value: &Value) -> (Option<CronType>, Option<String>) {
    if let Some(schedule) = value.get("schedule") {
        if let Some(text) = schedule.as_str().filter(|value| !value.trim().is_empty()) {
            return (Some(CronType::Cron), Some(text.trim().to_string()));
        }
        if let Some(kind) = schedule.get("kind").and_then(Value::as_str) {
            return match kind {
                "cron" => (
                    Some(CronType::Cron),
                    string_field(schedule, &["expr", "cron", "label"]),
                ),
                "every" | "interval" => (
                    Some(CronType::Every),
                    scalar_field(schedule, &["everyMs", "every", "intervalMs", "label"]),
                ),
                "at" => (
                    Some(CronType::At),
                    scalar_field(schedule, &["at", "runAt", "label"]),
                ),
                "rrule" => (
                    Some(CronType::Rrule),
                    string_field(schedule, &["rrule", "expr", "label"]),
                ),
                _ => (None, string_field(schedule, &["label"])),
            };
        }
        if let Some(expr) = string_field(schedule, &["expr", "cron"]) {
            return (Some(CronType::Cron), Some(expr));
        }
        if let Some(every) = scalar_field(schedule, &["everyMs", "every", "intervalMs"]) {
            return (Some(CronType::Every), Some(every));
        }
        if let Some(at) = scalar_field(schedule, &["at", "runAt"]) {
            return (Some(CronType::At), Some(at));
        }
    }
    if let Some(cron) = string_field(value, &["cron"]) {
        return (Some(CronType::Cron), Some(cron));
    }
    if let Some(rrule) = string_field(value, &["rrule"]) {
        return (Some(CronType::Rrule), Some(rrule));
    }
    if let Some(every) = scalar_field(value, &["everyMs", "every", "intervalMs"]) {
        return (Some(CronType::Every), Some(every));
    }
    if let Some(at) = scalar_field(value, &["at", "runAt", "nextRunAt"]) {
        return (Some(CronType::At), Some(at));
    }
    (None, None)
}

fn cwds(value: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(cwd) = string_field(value, &["cwd"]) {
        out.push(cwd);
    }
    if let Some(items) = value.get("contextDirs").and_then(Value::as_array) {
        out.extend(items.iter().filter_map(Value::as_str).map(str::to_string));
    }
    if let Some(items) = value.get("additionalDirectories").and_then(Value::as_array) {
        out.extend(items.iter().filter_map(Value::as_str).map(str::to_string));
    }
    if let Some(path) = value
        .get("workspace")
        .and_then(|workspace| string_field(workspace, &["path", "cwd"]))
    {
        out.push(path);
    }
    if let Some(path) = value
        .get("project")
        .and_then(|project| project.get("path"))
        .and_then(Value::as_str)
    {
        out.push(path.to_string());
    }
    out
}

fn dedup_crons(items: Vec<CronData>) -> Vec<CronData> {
    let mut out = Vec::new();
    for item in items {
        if out.iter().any(|seen: &CronData| seen.id == item.id) {
            continue;
        }
        out.push(item);
    }
    out
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn scalar_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        let value = value.get(*key)?;
        match value {
            Value::String(value) if !value.trim().is_empty() => Some(value.trim().to_string()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        }
    })
}

fn timestamp(value: Option<&Value>) -> Option<f64> {
    value.and_then(|value| {
        value.as_f64().or_else(|| {
            value
                .as_str()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.timestamp_millis() as f64)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_connector_cron_task_list() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("cron.json"),
            serde_json::to_vec(&serde_json::json!({
                "success": true,
                "data": {
                    "tasks": [
                        {
                            "id": "task-1",
                            "name": "Daily report",
                            "description": "fallback",
                            "enabled": true,
                            "schedule": {
                                "kind": "cron",
                                "expr": "0 9 * * *",
                                "tz": "Asia/Shanghai"
                            },
                            "prompt": "prepare report",
                            "contextDirs": ["C:/workspace"],
                            "createdAt": "2026-07-22T00:00:00Z"
                        }
                    ]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let tasks = cron_data("qoder-work", dir.path()).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "task-1");
        assert_eq!(tasks[0].name, "Daily report");
        assert_eq!(tasks[0].prompt, "prepare report");
        assert_eq!(tasks[0].schedule.as_deref(), Some("0 9 * * *"));
        assert_eq!(tasks[0].cron_type, Some(CronType::Cron));
        assert_eq!(tasks[0].cwds, ["C:/workspace"]);
        assert!(tasks[0].created_at.is_some());
    }

    #[test]
    fn reads_every_and_at_schedule_shapes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("tasks.json"),
            serde_json::to_vec(&serde_json::json!({
                "tasks": [
                    {"id": "every", "schedule": {"kind": "every", "everyMs": 60000}},
                    {"id": "at", "schedule": {"kind": "at", "at": "2026-07-22T00:00:00Z"}}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let tasks = cron_data("qoder-work", dir.path()).unwrap();

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].cron_type, Some(CronType::Every));
        assert_eq!(tasks[0].schedule.as_deref(), Some("60000"));
        assert_eq!(tasks[1].cron_type, Some(CronType::At));
    }

    #[test]
    fn reads_scheduled_tasks_from_qoderwork_database() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".qoderwork");
        let app_root = dir.path().join("AppData").join("Roaming").join("QoderWork");
        std::fs::create_dir_all(&agent_home).unwrap();
        std::fs::create_dir_all(app_root.join("data")).unwrap();
        let database = rusqlite::Connection::open(app_root.join("data").join("agents.db")).unwrap();
        database
            .execute(
                "CREATE TABLE scheduled_tasks (
                    id text PRIMARY KEY NOT NULL,
                    name text NOT NULL,
                    description text,
                    enabled integer NOT NULL,
                    schedule text NOT NULL,
                    payload text NOT NULL,
                    created_at integer,
                    updated_at integer,
                    deleted_at integer
                )",
                [],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO scheduled_tasks
                 (id, name, description, enabled, schedule, payload, created_at, updated_at, deleted_at)
                 VALUES
                 ('task-db', 'Database task', NULL, 1,
                  '{\"kind\":\"cron\",\"expr\":\"30 9 * * *\",\"tz\":\"Asia/Shanghai\"}',
                  '{\"kind\":\"agentTurn\",\"message\":\"prepare report\",\"cwd\":\"C:/workspace\"}',
                  1784780299, 1784780308, NULL)",
                [],
            )
            .unwrap();

        let tasks = cron_data("qoder-work", &agent_home).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "task-db");
        assert_eq!(tasks[0].name, "Database task");
        assert_eq!(tasks[0].prompt, "prepare report");
        assert_eq!(tasks[0].schedule.as_deref(), Some("30 9 * * *"));
        assert_eq!(tasks[0].cron_type, Some(CronType::Cron));
        assert_eq!(tasks[0].enabled, true);
        assert_eq!(tasks[0].cwds, ["C:/workspace"]);
        assert_eq!(tasks[0].created_at, Some(1784780299.0));
    }
}
