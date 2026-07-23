use std::path::Path;

use rusqlite::types::Value as SqlValue;
use serde_json::Value;

use crate::SentraResult;
use crate::interfaces::{CronData, CronType};
use crate::utils::SqliteDatabase;

const AUTOMATION_COLUMNS: &[&str] = &[
    "id",
    "name",
    "prompt",
    "status",
    "schedule_type",
    "next_run_at",
    "last_run_at",
    "cwds",
    "rrule",
    "scheduled_at",
    "created_at",
    "updated_at",
    "deleted_at",
];

pub(super) fn automation_database_crons(database_path: &Path) -> SentraResult<Vec<CronData>> {
    let Some(database) = SqliteDatabase::open_read_only(database_path)? else {
        return Ok(Vec::new());
    };
    if !database.table_exists("automations")? {
        return Ok(Vec::new());
    }
    let sql = automation_select_sql(&database)?;
    let tasks = database.query_map(&sql, rusqlite::params![], |row| {
        Ok(DatabaseAutomation {
            id: row_string(row, 0)?,
            name: row_string(row, 1)?,
            prompt: row_string(row, 2)?,
            status: row_string(row, 3)?,
            schedule_type: row_string(row, 4)?,
            next_run_at: row_string(row, 5)?,
            last_run_at: row_string(row, 6)?,
            cwds: row_string(row, 7)?,
            rrule: row_string(row, 8)?,
            scheduled_at: row_string(row, 9)?,
            created_at: row_string(row, 10)?,
            updated_at: row_string(row, 11)?,
            deleted_at: row_string(row, 12)?,
        })
    })?;
    Ok(dedup_crons(
        tasks
            .into_iter()
            .filter_map(|task| cron_from_database_automation(task, database_path))
            .collect(),
    ))
}

fn automation_select_sql(database: &SqliteDatabase) -> SentraResult<String> {
    let columns = database.query_map(
        "PRAGMA table_info(automations)",
        rusqlite::params![],
        |row| row.get::<_, String>(1),
    )?;
    let selected = AUTOMATION_COLUMNS
        .iter()
        .map(|column| {
            if columns
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(column))
            {
                (*column).to_string()
            } else {
                format!("NULL AS {column}")
            }
        })
        .collect::<Vec<_>>();
    Ok(format!("SELECT {} FROM automations", selected.join(", ")))
}

#[derive(Debug)]
struct DatabaseAutomation {
    id: Option<String>,
    name: Option<String>,
    prompt: Option<String>,
    status: Option<String>,
    schedule_type: Option<String>,
    next_run_at: Option<String>,
    last_run_at: Option<String>,
    cwds: Option<String>,
    rrule: Option<String>,
    scheduled_at: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    deleted_at: Option<String>,
}

fn cron_from_database_automation(
    task: DatabaseAutomation,
    database_path: &Path,
) -> Option<CronData> {
    if text_present(task.deleted_at.as_deref()) {
        return None;
    }
    let id = clean_string(task.id.as_deref())?;
    let (cron_type, schedule) = database_schedule(&task);
    Some(CronData {
        id: id.clone(),
        name: task.name.unwrap_or_else(|| id.clone()),
        prompt: task.prompt.unwrap_or_default(),
        enabled: automation_enabled(task.status.as_deref()),
        home: Some(database_path.to_path_buf()),
        cron_type,
        schedule,
        cwds: database_cwds(task.cwds.as_deref()),
        created_at: timestamp_str(task.created_at.as_deref()),
        updated_at: timestamp_str(task.updated_at.as_deref())
            .or_else(|| timestamp_str(task.last_run_at.as_deref())),
        files: Vec::new(),
    })
}

fn database_schedule(task: &DatabaseAutomation) -> (Option<CronType>, Option<String>) {
    if let Some(rrule) = clean_string(task.rrule.as_deref()) {
        return (Some(CronType::Rrule), Some(rrule));
    }
    if let Some(at) = clean_string(task.scheduled_at.as_deref()) {
        return (Some(CronType::At), Some(at));
    }
    if let Some(next_run_at) = clean_string(task.next_run_at.as_deref()) {
        return (Some(CronType::At), Some(next_run_at));
    }
    let Some(schedule_type) = clean_string(task.schedule_type.as_deref()) else {
        return (None, None);
    };
    match schedule_type.to_ascii_lowercase().as_str() {
        "cron" => (Some(CronType::Cron), None),
        "every" | "interval" | "recurring" => (Some(CronType::Every), Some(schedule_type)),
        "at" | "once" | "scheduled" => (Some(CronType::At), Some(schedule_type)),
        _ => (None, Some(schedule_type)),
    }
}

fn automation_enabled(status: Option<&str>) -> bool {
    !matches!(
        status.map(|value| value.trim().to_ascii_lowercase()),
        Some(value)
            if matches!(
                value.as_str(),
                "disabled" | "cancelled" | "canceled" | "deleted" | "paused"
            )
    )
}

fn database_cwds(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Vec::new();
    };
    if let Ok(value) = serde_json::from_str::<Value>(raw) {
        return cwds(&value);
    }
    vec![raw.to_string()]
}

fn cwds(value: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(items) = value.as_array() {
        out.extend(items.iter().filter_map(Value::as_str).map(str::to_string));
        return out;
    }
    if let Some(cwd) = value.as_str().and_then(|value| clean_string(Some(value))) {
        out.push(cwd);
        return out;
    }
    if let Some(cwd) = string_field(value, &["cwd", "workdir"]) {
        out.push(cwd);
    }
    for key in [
        "cwds",
        "contextDirs",
        "additionalDirectories",
        "workspaceFolders",
    ] {
        if let Some(items) = value.get(key).and_then(Value::as_array) {
            out.extend(items.iter().filter_map(Value::as_str).map(str::to_string));
        }
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
        .and_then(|value| clean_string(Some(value)))
}

fn timestamp_str(value: Option<&str>) -> Option<f64> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    value.parse::<f64>().ok().or_else(|| {
        chrono::DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|value| value.timestamp_millis() as f64)
    })
}

fn text_present(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

fn clean_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn row_string(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<String>> {
    let value = row.get::<_, Option<SqlValue>>(index)?;
    Ok(sql_value_to_string(value))
}

fn sql_value_to_string(value: Option<SqlValue>) -> Option<String> {
    let value = match value? {
        SqlValue::Null => return None,
        SqlValue::Integer(value) => value.to_string(),
        SqlValue::Real(value) => value.to_string(),
        SqlValue::Text(value) => value,
        SqlValue::Blob(_) => return None,
    };
    clean_string(Some(&value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_automation_database_without_deleted_rows() {
        let dir = tempfile::tempdir().unwrap();
        let database_path = dir.path().join("automations.db");
        let database = rusqlite::Connection::open(&database_path).unwrap();
        create_automations_table(&database);
        database
            .execute(
                "INSERT INTO automations
                 (id, name, prompt, status, schedule_type, next_run_at, last_run_at, cwds, rrule,
                  scheduled_at, created_at, updated_at, deleted_at)
                 VALUES
                 ('task-1', 'Morning task', 'ship it', 'enabled', 'recurring',
                  '2026-07-23T09:00:00Z', NULL, '[\"C:/work\"]',
                  'FREQ=DAILY;BYHOUR=9', NULL, 1784780299, 1784780308, NULL),
                 ('deleted', 'Deleted', 'skip', 'enabled', 'once',
                  NULL, NULL, NULL, NULL, '2026-07-23T10:00:00Z', NULL, NULL, 1)",
                [],
            )
            .unwrap();

        let tasks = automation_database_crons(&database_path).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "task-1");
        assert_eq!(tasks[0].name, "Morning task");
        assert_eq!(tasks[0].prompt, "ship it");
        assert_eq!(tasks[0].enabled, true);
        assert_eq!(tasks[0].cron_type, Some(CronType::Rrule));
        assert_eq!(tasks[0].schedule.as_deref(), Some("FREQ=DAILY;BYHOUR=9"));
        assert_eq!(tasks[0].cwds, ["C:/work"]);
        assert_eq!(tasks[0].created_at, Some(1784780299.0));
    }

    #[test]
    fn reads_real_codebuddy_ide_schema_without_deleted_at_column() {
        let dir = tempfile::tempdir().unwrap();
        let database_path = dir.path().join("automations.db");
        let database = rusqlite::Connection::open(&database_path).unwrap();
        database
            .execute(
                "CREATE TABLE automations (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    prompt TEXT NOT NULL,
                    status TEXT NOT NULL,
                    schedule_type TEXT NOT NULL DEFAULT 'recurring',
                    next_run_at INTEGER,
                    last_run_at INTEGER,
                    cwds TEXT NOT NULL DEFAULT '[]',
                    rrule TEXT NOT NULL DEFAULT '',
                    scheduled_at TEXT,
                    valid_from TEXT,
                    valid_until TEXT,
                    skills_json TEXT NOT NULL DEFAULT '[]',
                    model_id TEXT,
                    model_is_thinking INTEGER NOT NULL DEFAULT 0,
                    last_conversation_id_map TEXT NOT NULL DEFAULT '{}',
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    push_to_wechat INTEGER NOT NULL DEFAULT 0
                )",
                [],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO automations
                 (id, name, prompt, status, schedule_type, next_run_at, last_run_at, cwds, rrule,
                  scheduled_at, valid_from, valid_until, skills_json, model_id, model_is_thinking,
                  last_conversation_id_map, created_at, updated_at, push_to_wechat)
                 VALUES
                 ('automation', 'Weekly summary', 'summarize news', 'ACTIVE', 'recurring',
                  1785113850000, NULL, '[\"C:/Users/me/CodeBuddy/workspace\"]',
                  'FREQ=WEEKLY;BYDAY=MO;BYHOUR=9;BYMINUTE=0', NULL, NULL, NULL,
                  '[]', 'hy3', 1, '{}', 1784787357839, 1784787357839, 0)",
                [],
            )
            .unwrap();

        let tasks = automation_database_crons(&database_path).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "automation");
        assert_eq!(tasks[0].name, "Weekly summary");
        assert_eq!(tasks[0].enabled, true);
        assert_eq!(tasks[0].cron_type, Some(CronType::Rrule));
        assert_eq!(
            tasks[0].schedule.as_deref(),
            Some("FREQ=WEEKLY;BYDAY=MO;BYHOUR=9;BYMINUTE=0")
        );
        assert_eq!(tasks[0].cwds, ["C:/Users/me/CodeBuddy/workspace"]);
    }

    fn create_automations_table(database: &rusqlite::Connection) {
        database
            .execute(
                "CREATE TABLE automations (
                    id text PRIMARY KEY,
                    name text,
                    prompt text,
                    status text,
                    schedule_type text,
                    next_run_at text,
                    last_run_at text,
                    cwds text,
                    rrule text,
                    scheduled_at text,
                    created_at,
                    updated_at,
                    deleted_at
                )",
                [],
            )
            .unwrap();
    }
}
