use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{Asset, AssetType, CronData, CronType};
use crate::utils::read_json_file;
use crate::SentraResult;

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
        let mut results = Vec::new();
        for manifest_path in super::plugin_manifests(self.core.agent_home()) {
            let Some(manifest) = read_json_file(&manifest_path)? else {
                continue;
            };
            collect_crons(&manifest, &manifest_path, &mut results);
        }
        Ok(results)
    }
}

fn collect_crons(value: &serde_json::Value, home: &std::path::Path, results: &mut Vec<CronData>) {
    for key in ["scheduled", "schedules", "cron", "tasks"] {
        let Some(raw) = value.get(key) else {
            continue;
        };
        if let Some(items) = raw.as_array() {
            for item in items {
                if let Some(cron) = cron_from_value(item, home) {
                    results.push(cron);
                }
            }
        } else if let Some(cron) = cron_from_value(raw, home) {
            results.push(cron);
        }
    }
}

fn cron_from_value(value: &serde_json::Value, home: &std::path::Path) -> Option<CronData> {
    let id = string_field(value, "id")
        .or_else(|| string_field(value, "name"))
        .or_else(|| string_field(value, "command"))?;
    let schedule = string_field(value, "cron")
        .or_else(|| string_field(value, "schedule"))
        .or_else(|| string_field(value, "rrule"));
    Some(CronData {
        id: id.clone(),
        name: string_field(value, "name").unwrap_or(id),
        prompt: string_field(value, "prompt")
            .or_else(|| string_field(value, "command"))
            .unwrap_or_default(),
        enabled: value
            .get("enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        home: Some(home.to_path_buf()),
        cron_type: schedule.as_deref().map(|schedule| {
            if schedule.starts_with("RRULE:") {
                CronType::Rrule
            } else {
                CronType::Cron
            }
        }),
        schedule,
        cwds: Vec::new(),
        created_at: None,
        updated_at: None,
        files: Vec::new(),
    })
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
