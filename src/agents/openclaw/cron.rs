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
    let cron_dir = agent_home.join("cron");
    let Some(data) = read_json_file(cron_dir.join("jobs.json"))? else {
        return Ok(Vec::new());
    };
    let states = read_json_file(cron_dir.join("jobs-state.json"))?.unwrap_or_default();
    let Some(jobs) = data.get("jobs").and_then(|value| value.as_array()) else {
        return Ok(Vec::new());
    };
    Ok(jobs
        .iter()
        .filter_map(|job| {
            let id = job.get("id").and_then(|value| value.as_str())?;
            let payload = job.get("payload").and_then(|value| value.as_object());
            let created_at = job.get("createdAtMs").and_then(|value| value.as_f64());
            let updated_at = states
                .get("jobs")
                .and_then(|jobs| jobs.get(id))
                .and_then(|state| state.get("updatedAtMs"))
                .and_then(|value| value.as_f64())
                .or(created_at);
            let (cron_type, schedule) = schedule_of(job.get("schedule"));
            Some(CronData {
                id: id.to_string(),
                name: job
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or(id)
                    .to_string(),
                prompt: payload
                    .and_then(|payload| {
                        payload
                            .get("text")
                            .or_else(|| payload.get("message"))
                            .or_else(|| payload.get("prompt"))
                    })
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                enabled: job
                    .get("enabled")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
                home: None,
                cron_type,
                schedule,
                cwds: cwds_of(payload),
                created_at,
                updated_at,
                files: Vec::new(),
            })
        })
        .collect())
}

fn schedule_of(schedule: Option<&serde_json::Value>) -> (Option<CronType>, Option<String>) {
    let Some(schedule) = schedule else {
        return (None, None);
    };
    match schedule.get("kind").and_then(|value| value.as_str()) {
        Some("cron") => (
            Some(CronType::Cron),
            schedule
                .get("expr")
                .and_then(|value| value.as_str())
                .map(str::to_string),
        ),
        Some("rrule") => (
            Some(CronType::Rrule),
            schedule
                .get("rrule")
                .and_then(|value| value.as_str())
                .map(str::to_string),
        ),
        Some("every") => (
            Some(CronType::Every),
            schedule
                .get("every")
                .and_then(|value| value.as_str())
                .map(str::to_string),
        ),
        Some("at") => (
            Some(CronType::At),
            schedule
                .get("at")
                .and_then(|value| value.as_str())
                .map(str::to_string),
        ),
        _ => (None, None),
    }
}

fn cwds_of(payload: Option<&serde_json::Map<String, serde_json::Value>>) -> Vec<String> {
    let Some(payload) = payload else {
        return Vec::new();
    };
    payload
        .get("cwds")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .or_else(|| {
            payload
                .get("cwd")
                .and_then(|value| value.as_str())
                .map(|value| vec![value.to_string()])
        })
        .unwrap_or_default()
}
