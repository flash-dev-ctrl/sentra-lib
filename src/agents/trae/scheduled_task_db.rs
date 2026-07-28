use std::path::{Path, PathBuf};

use rusqlite::types::Value as SqlValue;
use serde_json::Value;

use crate::interfaces::{CronData, CronType};
use crate::utils::SqliteDatabase;
use crate::{SentraError, SentraResult};

const SCHEDULED_TASK_COLUMNS: &[&str] = &[
    "scheduled_task_id",
    "id",
    "task_id",
    "name",
    "title",
    "description",
    "trigger_type",
    "trigger_config",
    "cron_expression",
    "schedule",
    "rrule",
    "prompt_template",
    "prompt",
    "status",
    "enabled",
    "stopped_at",
    "disabled_reason",
    "max_execution_count",
    "execution_count",
    "next_run_at",
    "last_run_at",
    "local_project_id",
    "user_context",
    "created_at",
    "updated_at",
    "deleted_at",
];

pub(super) fn cron_data(app_roots: Vec<PathBuf>) -> SentraResult<Vec<CronData>> {
    let mut out = Vec::new();
    for app_root in app_roots {
        let database_path = app_root
            .join("ModularData")
            .join("ai-agent")
            .join("database.db");
        match database_crons(&database_path) {
            Ok(crons) => out.extend(crons),
            Err(err) if is_unreadable_database(&err) => continue,
            Err(err) => return Err(err),
        }
    }
    Ok(dedup_crons(out))
}

fn database_crons(database_path: &Path) -> SentraResult<Vec<CronData>> {
    let Some(database) = SqliteDatabase::open_read_only(database_path)? else {
        return Ok(Vec::new());
    };
    if !database.table_exists("scheduled_tasks")? {
        return Ok(Vec::new());
    }
    let sql = scheduled_task_select_sql(&database)?;
    let tasks = database.query_map(&sql, rusqlite::params![], |row| {
        Ok(DatabaseScheduledTask {
            scheduled_task_id: row_string(row, 0)?,
            id: row_string(row, 1)?,
            task_id: row_string(row, 2)?,
            name: row_string(row, 3)?,
            title: row_string(row, 4)?,
            description: row_string(row, 5)?,
            trigger_type: row_string(row, 6)?,
            trigger_config: row_string(row, 7)?,
            cron_expression: row_string(row, 8)?,
            schedule: row_string(row, 9)?,
            rrule: row_string(row, 10)?,
            prompt_template: row_string(row, 11)?,
            prompt: row_string(row, 12)?,
            status: row_string(row, 13)?,
            enabled: row_string(row, 14)?,
            stopped_at: row_string(row, 15)?,
            disabled_reason: row_string(row, 16)?,
            max_execution_count: row_string(row, 17)?,
            execution_count: row_string(row, 18)?,
            next_run_at: row_string(row, 19)?,
            last_run_at: row_string(row, 20)?,
            local_project_id: row_string(row, 21)?,
            user_context: row_string(row, 22)?,
            created_at: row_string(row, 23)?,
            updated_at: row_string(row, 24)?,
            deleted_at: row_string(row, 25)?,
        })
    })?;
    Ok(dedup_crons(
        tasks
            .into_iter()
            .filter_map(|task| cron_from_database_task(task, database_path))
            .collect(),
    ))
}

fn is_unreadable_database(error: &SentraError) -> bool {
    let SentraError::Sqlite { source, .. } = error else {
        return false;
    };
    matches!(
        source.sqlite_error_code(),
        Some(rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt)
    )
}

fn scheduled_task_select_sql(database: &SqliteDatabase) -> SentraResult<String> {
    let columns = database.query_map(
        "PRAGMA table_info(scheduled_tasks)",
        rusqlite::params![],
        |row| row.get::<_, String>(1),
    )?;
    let selected = SCHEDULED_TASK_COLUMNS
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
    Ok(format!(
        "SELECT {} FROM scheduled_tasks",
        selected.join(", ")
    ))
}

#[derive(Debug)]
struct DatabaseScheduledTask {
    scheduled_task_id: Option<String>,
    id: Option<String>,
    task_id: Option<String>,
    name: Option<String>,
    title: Option<String>,
    description: Option<String>,
    trigger_type: Option<String>,
    trigger_config: Option<String>,
    cron_expression: Option<String>,
    schedule: Option<String>,
    rrule: Option<String>,
    prompt_template: Option<String>,
    prompt: Option<String>,
    status: Option<String>,
    enabled: Option<String>,
    stopped_at: Option<String>,
    disabled_reason: Option<String>,
    max_execution_count: Option<String>,
    execution_count: Option<String>,
    next_run_at: Option<String>,
    last_run_at: Option<String>,
    local_project_id: Option<String>,
    user_context: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    deleted_at: Option<String>,
}

fn cron_from_database_task(task: DatabaseScheduledTask, database_path: &Path) -> Option<CronData> {
    if text_present(task.deleted_at.as_deref()) {
        return None;
    }
    let id = clean_string(task.scheduled_task_id.as_deref())
        .or_else(|| clean_string(task.id.as_deref()))
        .or_else(|| clean_string(task.task_id.as_deref()))?;
    let (cron_type, schedule) = database_schedule(&task);
    Some(CronData {
        id: id.clone(),
        name: clean_string(task.name.as_deref())
            .or_else(|| clean_string(task.title.as_deref()))
            .unwrap_or_else(|| id.clone()),
        prompt: database_prompt(&task).unwrap_or_default(),
        enabled: task_enabled(&task),
        home: Some(database_path.to_path_buf()),
        cron_type,
        schedule,
        cwds: database_cwds(
            task.user_context.as_deref(),
            task.local_project_id.as_deref(),
        ),
        created_at: timestamp_str(task.created_at.as_deref()),
        updated_at: timestamp_str(task.updated_at.as_deref())
            .or_else(|| timestamp_str(task.last_run_at.as_deref())),
        files: Vec::new(),
    })
}

fn database_prompt(task: &DatabaseScheduledTask) -> Option<String> {
    for raw in [
        task.prompt_template.as_deref(),
        task.prompt.as_deref(),
        task.description.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(prompt) = prompt_from_raw(raw) {
            return Some(prompt);
        }
    }
    None
}

fn prompt_from_raw(raw: &str) -> Option<String> {
    let raw = clean_string(Some(raw))?;
    if let Ok(value) = serde_json::from_str::<Value>(&raw) {
        return string_field(
            &value,
            &[
                "prompt",
                "prompt_template",
                "template",
                "text",
                "content",
                "message",
                "query",
                "description",
            ],
        )
        .or_else(|| value.as_str().and_then(|value| clean_string(Some(value))))
        .or(Some(raw));
    }
    Some(raw)
}

fn database_schedule(task: &DatabaseScheduledTask) -> (Option<CronType>, Option<String>) {
    if let Some(rrule) = clean_string(task.rrule.as_deref()) {
        return (Some(CronType::Rrule), Some(rrule));
    }
    if let Some(cron) = clean_string(task.cron_expression.as_deref()) {
        return (Some(CronType::Cron), Some(cron));
    }
    if let Some((cron_type, schedule)) = task.schedule.as_deref().and_then(schedule_from_raw) {
        if cron_type.is_some() || schedule.is_some() {
            return (cron_type, schedule);
        }
    }
    if let Some((cron_type, schedule)) = task.trigger_config.as_deref().and_then(schedule_from_raw)
    {
        if cron_type.is_some() || schedule.is_some() {
            if cron_type.is_some() {
                return (cron_type, schedule);
            }
            if let Some(trigger_type) = task
                .trigger_type
                .as_deref()
                .and_then(cron_type_from_trigger_type)
            {
                return (Some(trigger_type), schedule);
            }
        }
    }
    if let Some(trigger_type) = task
        .trigger_type
        .as_deref()
        .and_then(cron_type_from_trigger_type)
    {
        return (
            Some(trigger_type),
            trigger_config_label(task.trigger_config.as_deref()).or_else(|| {
                if trigger_type == CronType::At {
                    clean_string(task.next_run_at.as_deref())
                } else {
                    None
                }
            }),
        );
    }
    if let Some(next_run_at) = clean_string(task.next_run_at.as_deref()) {
        return (Some(CronType::At), Some(next_run_at));
    }
    (None, None)
}

fn schedule_from_raw(raw: &str) -> Option<(Option<CronType>, Option<String>)> {
    let raw = clean_string(Some(raw))?;
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return Some((None, Some(raw)));
    };
    Some(schedule_from_value(&value))
}

fn schedule_from_value(value: &Value) -> (Option<CronType>, Option<String>) {
    if let Some(text) = value.as_str().and_then(|value| clean_string(Some(value))) {
        return (None, Some(text));
    }
    if let Some(rrule) = string_field(value, &["rrule"]) {
        return (Some(CronType::Rrule), Some(rrule));
    }
    if let Some(cron) = string_field(
        value,
        &[
            "cron_expression",
            "cronExpression",
            "cron",
            "expr",
            "expression",
        ],
    ) {
        return (Some(CronType::Cron), Some(cron));
    }
    if let Some(at) = scalar_field(
        value,
        &["at", "run_at", "runAt", "scheduled_at", "scheduledAt"],
    ) {
        return (Some(CronType::At), Some(at));
    }
    if let Some(every) = scalar_field(
        value,
        &[
            "every",
            "every_ms",
            "everyMs",
            "interval",
            "interval_ms",
            "intervalMs",
        ],
    ) {
        return (Some(CronType::Every), Some(every));
    }
    if let Some(schedule) = value.get("schedule") {
        let nested = schedule_from_value(schedule);
        if nested.0.is_some() || nested.1.is_some() {
            return nested;
        }
    }
    if let Some(kind) = string_field(value, &["kind", "type", "trigger_type", "triggerType"]) {
        return (
            cron_type_from_trigger_type(&kind),
            scalar_field(value, &["value", "label", "schedule"]),
        );
    }
    (None, scalar_field(value, &["value", "label", "schedule"]))
}

fn trigger_config_label(raw: Option<&str>) -> Option<String> {
    let raw = clean_string(raw)?;
    if let Ok(value) = serde_json::from_str::<Value>(&raw) {
        let (_, schedule) = schedule_from_value(&value);
        return schedule;
    }
    Some(raw)
}

fn cron_type_from_trigger_type(value: &str) -> Option<CronType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cron" | "crontab" | "recurring" | "schedule" | "scheduled" => Some(CronType::Cron),
        "rrule" => Some(CronType::Rrule),
        "every" | "interval" | "timer" => Some(CronType::Every),
        "at" | "once" | "one_time" | "one-time" | "single" => Some(CronType::At),
        _ => None,
    }
}

fn task_enabled(task: &DatabaseScheduledTask) -> bool {
    if bool_text(task.enabled.as_deref()).is_some_and(|value| !value) {
        return false;
    }
    if text_present(task.disabled_reason.as_deref()) || text_present(task.stopped_at.as_deref()) {
        return false;
    }
    if matches!(
        task.status
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase),
        Some(value)
            if matches!(
                value.as_str(),
                "disabled" | "cancelled" | "canceled" | "deleted" | "paused" | "stopped"
            )
    ) {
        return false;
    }
    let max_execution_count = int_str(task.max_execution_count.as_deref());
    let execution_count = int_str(task.execution_count.as_deref());
    if matches!((max_execution_count, execution_count), (Some(max), Some(count)) if max > 0 && count >= max)
    {
        return false;
    }
    true
}

fn database_cwds(user_context: Option<&str>, local_project_id: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(raw) = user_context.and_then(|value| clean_string(Some(value))) {
        if let Ok(value) = serde_json::from_str::<Value>(&raw) {
            out.extend(cwds_from_value(&value));
        } else if looks_like_path(&raw) {
            out.push(raw);
        }
    }
    if let Some(project) = local_project_id
        .and_then(|value| clean_string(Some(value)))
        .filter(|value| looks_like_path(value))
    {
        out.push(project);
    }
    dedup_strings(out)
}

fn cwds_from_value(value: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(path) = value.as_str().filter(|value| looks_like_path(value)) {
        out.push(path.to_string());
        return out;
    }
    for key in [
        "cwd",
        "workdir",
        "workspace",
        "workspacePath",
        "workspace_path",
        "projectPath",
        "project_path",
        "rootPath",
        "root_path",
        "path",
    ] {
        if let Some(path) = string_field(value, &[key]).filter(|value| looks_like_path(value)) {
            out.push(path);
        }
    }
    for key in [
        "cwds",
        "dirs",
        "paths",
        "contextDirs",
        "context_dirs",
        "additionalDirectories",
        "additional_directories",
        "workspaceFolders",
        "workspace_folders",
    ] {
        if let Some(items) = value.get(key).and_then(Value::as_array) {
            out.extend(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|value| looks_like_path(value))
                    .map(str::to_string),
            );
        }
    }
    for key in ["workspace", "project", "repository", "repo"] {
        if let Some(nested) = value.get(key) {
            out.extend(cwds_from_value(nested));
        }
    }
    out
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .and_then(|value| clean_string(Some(value)))
}

fn scalar_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        let value = value.get(*key)?;
        match value {
            Value::String(value) => clean_string(Some(value)),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        }
    })
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

fn int_str(value: Option<&str>) -> Option<i64> {
    value?.trim().parse::<i64>().ok()
}

fn bool_text(value: Option<&str>) -> Option<bool> {
    match value?.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "enabled" | "active" => Some(true),
        "0" | "false" | "no" | "off" | "disabled" | "inactive" => Some(false),
        _ => None,
    }
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

fn looks_like_path(value: &str) -> bool {
    value.contains(':') || value.contains('/') || value.contains('\\')
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

fn dedup_strings(items: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        if out.iter().any(|seen| seen == &item) {
            continue;
        }
        out.push(item);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_scheduled_tasks_from_database() {
        let dir = tempfile::tempdir().unwrap();
        let app_root = dir.path().join("Trae Work");
        let database_path = app_root
            .join("ModularData")
            .join("ai-agent")
            .join("database.db");
        std::fs::create_dir_all(database_path.parent().unwrap()).unwrap();
        let database = create_database(&database_path);
        database
            .execute(
                "CREATE TABLE scheduled_tasks (
                    scheduled_task_id TEXT PRIMARY KEY,
                    name TEXT,
                    trigger_type TEXT,
                    trigger_config TEXT,
                    prompt_template TEXT,
                    enabled INTEGER,
                    next_run_at INTEGER,
                    last_run_at INTEGER,
                    disabled_reason TEXT,
                    local_project_id TEXT,
                    user_context TEXT,
                    model_name TEXT,
                    created_at INTEGER,
                    updated_at INTEGER,
                    deleted_at INTEGER
                )",
                [],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO scheduled_tasks
                 (scheduled_task_id, name, trigger_type, trigger_config, prompt_template, enabled,
                  next_run_at, last_run_at, disabled_reason, local_project_id, user_context,
                  model_name, created_at, updated_at, deleted_at)
                 VALUES
                 ('LFHTR.RR_S_Z_A', '总结字节热点新闻', 'cron',
                  '{\"cron_expression\":\"0 9 * * *\"}',
                  '{\"prompt\":\"summarize ByteDance hot news\"}', 1,
                  1785123000000, NULL, NULL, NULL,
                  '{\"cwd\":\"C:/workspace\"}', 'seed', 1785122989500, 1785122989600, NULL)",
                [],
            )
            .unwrap();
        drop(database);

        let tasks = cron_data(vec![app_root]).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "LFHTR.RR_S_Z_A");
        assert_eq!(tasks[0].name, "总结字节热点新闻");
        assert_eq!(tasks[0].prompt, "summarize ByteDance hot news");
        assert_eq!(tasks[0].enabled, true);
        assert_eq!(tasks[0].cron_type, Some(CronType::Cron));
        assert_eq!(tasks[0].schedule.as_deref(), Some("0 9 * * *"));
        assert_eq!(tasks[0].cwds, ["C:/workspace"]);
        assert_eq!(tasks[0].created_at, Some(1785122989500.0));
    }

    #[test]
    fn reads_schema_when_optional_columns_are_absent() {
        let dir = tempfile::tempdir().unwrap();
        let app_root = dir.path().join("Trae");
        let database_path = app_root
            .join("ModularData")
            .join("ai-agent")
            .join("database.db");
        std::fs::create_dir_all(database_path.parent().unwrap()).unwrap();
        let database = create_database(&database_path);
        database
            .execute(
                "CREATE TABLE scheduled_tasks (
                    scheduled_task_id TEXT PRIMARY KEY,
                    trigger_type TEXT,
                    trigger_config TEXT,
                    prompt_template TEXT
                )",
                [],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO scheduled_tasks
                 (scheduled_task_id, trigger_type, trigger_config, prompt_template)
                 VALUES ('task-min', 'once', '{\"at\":\"2026-07-27T09:00:00Z\"}', 'ship it')",
                [],
            )
            .unwrap();
        drop(database);

        let tasks = cron_data(vec![app_root]).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "task-min");
        assert_eq!(tasks[0].name, "task-min");
        assert_eq!(tasks[0].prompt, "ship it");
        assert_eq!(tasks[0].cron_type, Some(CronType::At));
        assert_eq!(tasks[0].schedule.as_deref(), Some("2026-07-27T09:00:00Z"));
    }

    #[test]
    fn skips_unreadable_database_roots() {
        let dir = tempfile::tempdir().unwrap();
        let bad_app_root = dir.path().join("Unreadable");
        let bad_database_path = bad_app_root
            .join("ModularData")
            .join("ai-agent")
            .join("database.db");
        std::fs::create_dir_all(bad_database_path.parent().unwrap()).unwrap();
        std::fs::write(&bad_database_path, b"not a sqlite database").unwrap();

        let good_app_root = dir.path().join("Readable");
        let good_database_path = good_app_root
            .join("ModularData")
            .join("ai-agent")
            .join("database.db");
        std::fs::create_dir_all(good_database_path.parent().unwrap()).unwrap();
        let database = create_database(&good_database_path);
        database
            .execute(
                "CREATE TABLE scheduled_tasks (
                    scheduled_task_id TEXT PRIMARY KEY,
                    name TEXT,
                    trigger_type TEXT,
                    trigger_config TEXT,
                    prompt_template TEXT
                )",
                [],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO scheduled_tasks
                 (scheduled_task_id, name, trigger_type, trigger_config, prompt_template)
                 VALUES ('cloud-local-shadow', 'local fallback', 'cron',
                         '{\"cron\":\"15 10 * * *\"}', 'keep reading')",
                [],
            )
            .unwrap();
        drop(database);

        let tasks = cron_data(vec![bad_app_root, good_app_root]).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "cloud-local-shadow");
        assert_eq!(tasks[0].schedule.as_deref(), Some("15 10 * * *"));
    }

    fn create_database(path: &Path) -> rusqlite::Connection {
        rusqlite::Connection::open(path).unwrap()
    }
}
