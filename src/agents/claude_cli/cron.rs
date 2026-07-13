use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, CronData, CronType};
use crate::utils::read_json_file;

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
    let task_path = agent_home.join("scheduled_tasks.json");
    let Some(config) = read_json_file(&task_path)? else {
        return Ok(Vec::new());
    };
    let Some(tasks) = config.get("tasks").and_then(|value| value.as_array()) else {
        return Ok(Vec::new());
    };
    let mut results = Vec::new();
    for task in tasks {
        let Some(id) = task.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        let schedule = task
            .get("cron")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        results.push(CronData {
            id: id.to_string(),
            name: id.to_string(),
            prompt: task
                .get("prompt")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            enabled: task
                .get("recurring")
                .and_then(|value| value.as_bool())
                .unwrap_or(true),
            home: Some(task_path.clone()),
            cron_type: schedule.as_ref().map(|_| CronType::Cron),
            schedule,
            cwds: Vec::new(),
            created_at: None,
            updated_at: None,
            files: Vec::new(),
        });
    }
    Ok(results)
}
