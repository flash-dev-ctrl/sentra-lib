use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, CronData, CronType};
use crate::utils::read_json_file;

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
        let mut out = Vec::new();
        let cwd = std::env::current_dir().unwrap_or_default();
        for settings in [
            self.core.agent_home().join("settings.json"),
            cwd.join(".qoder").join("settings.json"),
        ] {
            let Some(config) = read_json_file(&settings)? else {
                continue;
            };
            let tasks = config
                .get("cron")
                .or_else(|| config.get("tasks"))
                .and_then(serde_json::Value::as_array);
            out.extend(tasks.into_iter().flatten().filter_map(|task| {
                let id = task.get("id").or_else(|| task.get("name"))?.as_str()?;
                let schedule = task
                    .get("schedule")
                    .or_else(|| task.get("cron"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                Some(CronData {
                    id: id.to_string(),
                    name: id.to_string(),
                    prompt: task
                        .get("prompt")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    enabled: task
                        .get("enabled")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(true),
                    home: Some(settings.clone()),
                    cron_type: schedule.as_ref().map(|_| CronType::Cron),
                    schedule,
                    cwds: Vec::new(),
                    created_at: None,
                    updated_at: None,
                    files: Vec::new(),
                })
            }));
        }
        Ok(out)
    }
}
