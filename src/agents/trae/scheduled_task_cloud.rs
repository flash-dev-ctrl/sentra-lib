use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;
use walkdir::WalkDir;

use crate::SentraResult;
use crate::agents::trae::surface;
use crate::interfaces::{CronData, CronType};

const SCHEDULED_TASKS_API_MARKER: &[u8] = b"/api/remote/v1/scheduled_tasks";
const SCHEDULED_TASKS_KEY_MARKER: &[u8] = b"scheduled_tasks";
const SCHEDULED_TASK_ID_MARKER: &[u8] = b"scheduled_task_id";
const MAX_CACHE_FILE_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn cron_data(agent_name: &str, agent_home: &Path) -> Vec<CronData> {
    cron_data_from_app_roots(surface::work_data_roots(agent_name, agent_home)).unwrap_or_default()
}

fn cron_data_from_app_roots(app_roots: Vec<PathBuf>) -> SentraResult<Vec<CronData>> {
    let mut out = Vec::new();
    for cache_path in cache_files(cache_roots(app_roots)) {
        let Ok(bytes) = fs::read(&cache_path) else {
            continue;
        };
        for response in cached_cloud_responses(&cache_path, &bytes) {
            if let Ok(tasks) = parse_cloud_response(&response, &cache_path) {
                out.extend(tasks);
            }
        }
    }
    Ok(dedup_crons(out))
}

fn cache_roots(app_roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for app_root in app_roots {
        roots.extend([
            app_root.join("Cache").join("Cache_Data"),
            app_root.join("Network").join("Cache").join("Cache_Data"),
            app_root.join("Service Worker").join("CacheStorage"),
        ]);
        roots.extend(nested_cache_roots(&app_root.join("Partitions")));
    }
    dedup_paths(roots)
}

fn nested_cache_roots(root: &Path) -> Vec<PathBuf> {
    if !root.is_dir() {
        return Vec::new();
    }
    WalkDir::new(root)
        .max_depth(8)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .filter(|entry| is_cache_root(entry.path()))
        .map(walkdir::DirEntry::into_path)
        .collect()
}

fn is_cache_root(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("Cache_Data") || name.eq_ignore_ascii_case("CacheStorage")
        })
}

fn cache_files(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        out.extend(
            WalkDir::new(root)
                .max_depth(8)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
                .filter(|entry| {
                    entry
                        .metadata()
                        .map(|metadata| metadata.len() <= MAX_CACHE_FILE_BYTES)
                        .unwrap_or(false)
                })
                .map(walkdir::DirEntry::into_path),
        );
    }
    dedup_paths(out)
}

fn cached_cloud_responses(path: &Path, bytes: &[u8]) -> Vec<String> {
    let path_has_hint = path
        .to_string_lossy()
        .to_ascii_lowercase()
        .contains("scheduled_task");
    if !path_has_hint
        && !contains_bytes(bytes, SCHEDULED_TASKS_API_MARKER)
        && !contains_bytes(bytes, SCHEDULED_TASKS_KEY_MARKER)
        && !contains_bytes(bytes, SCHEDULED_TASK_ID_MARKER)
    {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(bytes);
    extract_json_objects(&text)
        .into_iter()
        .filter(|raw| is_probable_cloud_response(raw))
        .collect()
}

fn is_probable_cloud_response(raw: &str) -> bool {
    raw.contains("\"items\"")
        && (raw.contains("\"code\"") || raw.contains("\"data\"") || raw.contains("scheduled_task"))
}

fn extract_json_objects(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = text[cursor..].find('{') {
        let start = cursor + relative_start;
        let Some(end) = json_object_end(text, start) else {
            break;
        };
        out.push(text[start..end].to_string());
        cursor = end;
    }
    out
}

fn json_object_end(text: &str, start: usize) -> Option<usize> {
    let mut depth = 0_i64;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, character) in text[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + offset + character.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

#[derive(Debug, Deserialize)]
struct CloudScheduledTasksResponse {
    code: Option<i64>,
    data: Option<CloudScheduledTasksData>,
}

#[derive(Debug, Deserialize, Default)]
struct CloudScheduledTasksData {
    #[serde(default)]
    items: Vec<CloudScheduledTask>,
}

#[derive(Debug, Deserialize, Default)]
struct CloudScheduledTask {
    scheduled_task_id: Option<String>,
    id: Option<String>,
    task_id: Option<String>,
    name: Option<String>,
    title: Option<String>,
    description: Option<Value>,
    prompt_template: Option<Value>,
    prompt: Option<Value>,
    trigger_type: Option<String>,
    trigger_config: Option<Value>,
    cron_expression: Option<String>,
    schedule: Option<Value>,
    rrule: Option<String>,
    status: Option<String>,
    enabled: Option<Value>,
    next_run_at: Option<Value>,
    last_run_at: Option<Value>,
    local_project_id: Option<Value>,
    user_context: Option<Value>,
    created_at: Option<Value>,
    updated_at: Option<Value>,
    deleted_at: Option<Value>,
}

fn parse_cloud_response(raw: &str, source: &Path) -> SentraResult<Vec<CronData>> {
    let response = serde_json::from_str::<CloudScheduledTasksResponse>(raw)?;
    if response.code.is_some_and(|code| code != 0) {
        return Ok(Vec::new());
    }
    let data = response.data.unwrap_or_default();
    Ok(dedup_crons(
        data.items
            .into_iter()
            .filter_map(|task| cron_from_cloud_task(task, source))
            .collect(),
    ))
}

fn cron_from_cloud_task(task: CloudScheduledTask, source: &Path) -> Option<CronData> {
    if text_present_value(task.deleted_at.as_ref()) {
        return None;
    }
    let id = clean_string(task.scheduled_task_id.as_deref())
        .or_else(|| clean_string(task.id.as_deref()))
        .or_else(|| clean_string(task.task_id.as_deref()))?;
    let (cron_type, schedule) = cloud_schedule(&task);
    Some(CronData {
        id: id.clone(),
        name: clean_string(task.name.as_deref())
            .or_else(|| clean_string(task.title.as_deref()))
            .unwrap_or_else(|| id.clone()),
        prompt: cloud_prompt(&task).unwrap_or_default(),
        enabled: cloud_task_enabled(&task),
        home: Some(source.to_path_buf()),
        cron_type,
        schedule,
        cwds: cloud_cwds(task.user_context.as_ref(), task.local_project_id.as_ref()),
        created_at: timestamp_value(task.created_at.as_ref()),
        updated_at: timestamp_value(task.updated_at.as_ref())
            .or_else(|| timestamp_value(task.last_run_at.as_ref())),
        files: Vec::new(),
    })
}

fn cloud_prompt(task: &CloudScheduledTask) -> Option<String> {
    for value in [
        task.prompt_template.as_ref(),
        task.prompt.as_ref(),
        task.description.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(prompt) = prompt_from_value(value) {
            return Some(prompt);
        }
    }
    None
}

fn prompt_from_value(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str().and_then(|value| clean_string(Some(value))) {
        if let Ok(nested) = serde_json::from_str::<Value>(&text) {
            return prompt_from_value(&nested).or(Some(text));
        }
        return Some(text);
    }
    if let Some(items) = value.as_array() {
        let prompts = items
            .iter()
            .filter_map(prompt_from_value)
            .collect::<Vec<_>>();
        if !prompts.is_empty() {
            return Some(prompts.join("\n"));
        }
    }
    for key in [
        "prompt",
        "prompt_template",
        "template",
        "text",
        "content",
        "data",
        "message",
        "query",
        "description",
    ] {
        if let Some(prompt) = value.get(key).and_then(prompt_from_value) {
            return Some(prompt);
        }
    }
    None
}

fn cloud_schedule(task: &CloudScheduledTask) -> (Option<CronType>, Option<String>) {
    if let Some(rrule) = clean_string(task.rrule.as_deref()) {
        return (Some(CronType::Rrule), Some(rrule));
    }
    if let Some(cron) = clean_string(task.cron_expression.as_deref()) {
        return (Some(CronType::Cron), Some(cron));
    }
    if let Some(schedule) = task.schedule.as_ref().map(schedule_from_value) {
        if schedule.0.is_some() || schedule.1.is_some() {
            return schedule;
        }
    }
    if let Some(schedule) = task.trigger_config.as_ref().map(schedule_from_value) {
        if schedule.0.is_some() || schedule.1.is_some() {
            if schedule.0.is_some() {
                return schedule;
            }
            if let Some(trigger_type) = task
                .trigger_type
                .as_deref()
                .and_then(cron_type_from_trigger_type)
            {
                return (Some(trigger_type), schedule.1);
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
            task.next_run_at
                .as_ref()
                .and_then(value_string)
                .filter(|_| trigger_type == CronType::At),
        );
    }
    if let Some(next_run_at) = task.next_run_at.as_ref().and_then(value_string) {
        return (Some(CronType::At), Some(next_run_at));
    }
    (None, None)
}

fn schedule_from_value(value: &Value) -> (Option<CronType>, Option<String>) {
    if let Some(text) = value.as_str().and_then(|value| clean_string(Some(value))) {
        if let Ok(nested) = serde_json::from_str::<Value>(&text) {
            let schedule = schedule_from_value(&nested);
            if schedule.0.is_some() || schedule.1.is_some() {
                return schedule;
            }
        }
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

fn cron_type_from_trigger_type(value: &str) -> Option<CronType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cron" | "crontab" | "recurring" | "schedule" | "scheduled" => Some(CronType::Cron),
        "rrule" => Some(CronType::Rrule),
        "every" | "interval" | "timer" => Some(CronType::Every),
        "at" | "once" | "one_time" | "one-time" | "single" => Some(CronType::At),
        _ => None,
    }
}

fn cloud_task_enabled(task: &CloudScheduledTask) -> bool {
    if bool_value(task.enabled.as_ref()).is_some_and(|value| !value) {
        return false;
    }
    !matches!(
        task.status
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase),
        Some(value)
            if matches!(
                value.as_str(),
                "disabled" | "cancelled" | "canceled" | "deleted" | "paused" | "stopped"
            )
    )
}

fn cloud_cwds(user_context: Option<&Value>, local_project_id: Option<&Value>) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(value) = user_context {
        out.extend(cwds_from_value(value));
    }
    if let Some(path) = local_project_id
        .and_then(value_string)
        .filter(|value| looks_like_path(value))
    {
        out.push(path);
    }
    dedup_strings(out)
}

fn cwds_from_value(value: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(path) = value.as_str().filter(|value| looks_like_path(value)) {
        out.push(path.to_string());
        return out;
    }
    if let Some(items) = value.as_array() {
        out.extend(
            items
                .iter()
                .filter_map(value_string)
                .filter(|value| looks_like_path(value)),
        );
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
                    .filter_map(value_string)
                    .filter(|value| looks_like_path(value)),
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
    keys.iter()
        .find_map(|key| value.get(*key).and_then(value_string))
}

fn value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => clean_string(Some(value)),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn timestamp_value(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(value) => timestamp_str(Some(value)),
        _ => None,
    }
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

fn bool_value(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(value) => Some(*value),
        Value::Number(value) => value.as_i64().and_then(|value| match value {
            1 => Some(true),
            0 => Some(false),
            _ => None,
        }),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" | "enabled" | "active" => Some(true),
            "0" | "false" | "no" | "off" | "disabled" | "inactive" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn text_present_value(value: Option<&Value>) -> bool {
    value.and_then(value_string).is_some()
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

fn dedup_paths(items: Vec<PathBuf>) -> Vec<PathBuf> {
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
    use std::fs;

    use serde_json::json;

    use super::*;

    #[test]
    fn parses_cloud_scheduled_tasks_response() {
        let source = Path::new("storage.json");
        let raw = json!({
            "code": 0,
            "message": "success",
            "data": {
                "total": 1,
                "items": [{
                    "scheduled_task_id": "LFHTR.RR_S_Z_A",
                    "name": "总结字节热点新闻",
                    "enabled": true,
                    "trigger_type": "cron",
                    "trigger_config": { "cron": "1 0 * * *", "timezone": "Asia/Shanghai" },
                    "prompt_template": "[{\"type\":\"text\",\"data\":{\"content\":\"总结字节热点新闻\"}}]",
                    "user_context": { "cwd": "C:/workspace" },
                    "created_at": "2026-07-27T03:29:49Z",
                    "updated_at": "2026-07-27T05:47:12Z"
                }]
            }
        })
        .to_string();

        let tasks = parse_cloud_response(&raw, source).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "LFHTR.RR_S_Z_A");
        assert_eq!(tasks[0].name, "总结字节热点新闻");
        assert_eq!(tasks[0].prompt, "总结字节热点新闻");
        assert_eq!(tasks[0].enabled, true);
        assert_eq!(tasks[0].cron_type, Some(CronType::Cron));
        assert_eq!(tasks[0].schedule.as_deref(), Some("1 0 * * *"));
        assert_eq!(tasks[0].cwds, ["C:/workspace"]);
        assert_eq!(tasks[0].created_at, Some(1785122989000.0));
    }

    #[test]
    fn returns_empty_without_local_cache() {
        let dir = tempfile::tempdir().unwrap();

        let tasks = cron_data_from_app_roots(vec![dir.path().join("TRAE SOLO")]).unwrap();

        assert!(tasks.is_empty());
    }

    #[test]
    fn reads_cloud_tasks_from_chromium_http_cache() {
        let dir = tempfile::tempdir().unwrap();
        let app_root = dir.path().join("TRAE SOLO");
        let cache_path = app_root.join("Cache").join("Cache_Data").join("f_000001");
        fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        let response = json!({
            "code": 0,
            "data": {
                "items": [{
                    "scheduled_task_id": "cloud-task",
                    "name": "Cloud task",
                    "trigger_type": "cron",
                    "trigger_config": { "cron": "0 9 * * *" },
                    "prompt_template": "ship it"
                }]
            }
        })
        .to_string();
        fs::write(
            &cache_path,
            format!(
                "https://coresg-normal.trae.ai/api/remote/v1/scheduled_tasks?page_size=100&hide_empty=true\0HTTP/1.1 200 OK\r\n\r\n{response}\0"
            ),
        )
        .unwrap();

        let tasks = cron_data_from_app_roots(vec![app_root]).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "cloud-task");
        assert_eq!(tasks[0].schedule.as_deref(), Some("0 9 * * *"));
        assert_eq!(tasks[0].home.as_deref(), Some(cache_path.as_path()));
    }

    #[test]
    fn ignores_empty_cloud_task_cache() {
        let dir = tempfile::tempdir().unwrap();
        let app_root = dir.path().join("TRAE SOLO");
        let cache_path = app_root.join("Cache").join("Cache_Data").join("f_000001");
        fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        fs::write(
            &cache_path,
            "https://coresg-normal.trae.ai/api/remote/v1/scheduled_tasks?page_size=100&hide_empty=true\0{\"code\":0,\"data\":{\"items\":[]},\"message\":\"success\"}",
        )
        .unwrap();

        let tasks = cron_data_from_app_roots(vec![app_root]).unwrap();

        assert!(tasks.is_empty());
    }
}
