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
    let agents_home = crate::agents::kimi::app_daimon_home(agent_home).join("agents");
    let mut paths = Vec::new();
    for agent_dir in read_dir_paths(&agents_home) {
        let automations = agent_dir.join("blueprint").join("automations");
        for automation_dir in read_dir_paths(&automations) {
            let path = automation_dir.join("automation.json");
            if path.is_file() {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(&path).ok()?;
            let document = serde_json::from_str::<Value>(&content).ok()?;
            automation_from_document(&document, &path)
        })
        .collect()
}

fn automation_from_document(document: &Value, path: &Path) -> Option<CronData> {
    let automation = document.get("automation").unwrap_or(document);
    let id = string_field(automation, &["automationId", "id"])?;
    let trigger = automation.get("trigger")?;
    let (cron_type, schedule) = schedule_from_trigger(trigger)?;
    let execution = automation.get("execution").unwrap_or(&Value::Null);
    let input = automation.get("input").unwrap_or(&Value::Null);
    let prompt = string_field(execution, &["prompt"])
        .or_else(|| string_field(input, &["prompt"]))
        .or_else(|| string_field(automation, &["description"]))
        .unwrap_or_default();
    let cwds = execution
        .get("workspace")
        .and_then(|workspace| string_field(workspace, &["path", "cwd"]))
        .into_iter()
        .collect();

    Some(CronData {
        id: id.clone(),
        name: string_field(automation, &["title", "name"]).unwrap_or(id),
        prompt,
        enabled: automation
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        home: Some(path.to_path_buf()),
        cron_type: Some(cron_type),
        schedule: Some(schedule),
        cwds,
        created_at: timestamp(automation.get("createdAt")),
        updated_at: timestamp(automation.get("updatedAt")),
        files: Vec::new(),
    })
}

fn schedule_from_trigger(trigger: &Value) -> Option<(CronType, String)> {
    if let Some(schedule) = string_field(trigger, &["cron", "schedule"]) {
        return Some((CronType::Cron, schedule));
    }
    if let Some(schedule) = string_field(trigger, &["rrule"]) {
        return Some((CronType::Rrule, schedule));
    }
    if let Some(schedule) = scalar_field(trigger, &["at", "runAt", "onceAt"]) {
        return Some((CronType::At, schedule));
    }
    scalar_field(trigger, &["intervalMs", "everyMs", "interval", "every"])
        .map(|schedule| (CronType::Every, schedule))
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

fn read_dir_paths(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_scheduled_automations_and_ignores_manual_ones() {
        let dir = tempfile::tempdir().unwrap();
        let automations = dir
            .path()
            .join("daimon-share")
            .join("daimon")
            .join("agents")
            .join("main")
            .join("blueprint")
            .join("automations");
        write_automation(
            &automations.join("automation_scheduled"),
            serde_json::json!({
                "version": 1,
                "automation": {
                    "automationId": "automation_scheduled",
                    "title": "Daily report",
                    "description": "fallback prompt",
                    "enabled": true,
                    "trigger": {"kind": "schedule", "cron": "0 9 * * *", "timezone": "Asia/Shanghai"},
                    "execution": {"kind": "agent", "prompt": "prepare report", "workspace": {"path": "C:/workspace"}},
                    "createdAt": "2026-07-22T00:00:00Z",
                    "updatedAt": "2026-07-22T01:00:00Z"
                }
            }),
        );
        write_automation(
            &automations.join("automation_manual"),
            serde_json::json!({
                "version": 1,
                "automation": {
                    "automationId": "automation_manual",
                    "title": "Manual action",
                    "enabled": true,
                    "trigger": {"kind": "manual"}
                }
            }),
        );

        let tasks = cron_data(dir.path());

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "automation_scheduled");
        assert_eq!(tasks[0].name, "Daily report");
        assert_eq!(tasks[0].prompt, "prepare report");
        assert_eq!(tasks[0].schedule.as_deref(), Some("0 9 * * *"));
        assert_eq!(tasks[0].cron_type, Some(CronType::Cron));
        assert_eq!(tasks[0].cwds, ["C:/workspace"]);
        assert!(tasks[0].created_at.is_some());
        assert!(tasks[0].updated_at.is_some());
    }

    fn write_automation(dir: &Path, value: Value) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("automation.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
}
