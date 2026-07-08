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
    let Some(data) = read_json_file(agent_home.join("cron").join("jobs.json"))? else {
        return Ok(Vec::new());
    };
    let Some(jobs) = data.get("jobs").and_then(|value| value.as_array()) else {
        return Ok(Vec::new());
    };
    Ok(jobs
        .iter()
        .filter_map(|job| {
            let id = job.get("id").and_then(|value| value.as_str())?;
            let created_at = job
                .get("created_at")
                .and_then(|value| value.as_str())
                .and_then(parse_timestamp_ms);
            let (cron_type, schedule) = schedule_of(job.get("schedule"));
            Some(CronData {
                id: id.to_string(),
                name: job
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or(id)
                    .to_string(),
                prompt: job
                    .get("prompt")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                enabled: job
                    .get("enabled")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true)
                    && !matches!(
                        job.get("state").and_then(|value| value.as_str()),
                        Some("paused" | "disabled")
                    ),
                home: None,
                cron_type,
                schedule,
                cwds: job
                    .get("workdir")
                    .and_then(|value| value.as_str())
                    .map(|value| vec![value.to_string()])
                    .unwrap_or_default(),
                created_at,
                updated_at: created_at,
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
        Some("interval") => (
            Some(CronType::Every),
            schedule
                .get("display")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .or_else(|| {
                    schedule
                        .get("minutes")
                        .and_then(|value| value.as_i64())
                        .map(|minutes| format!("{minutes}m"))
                }),
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

fn parse_timestamp_ms(value: &str) -> Option<f64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.timestamp_millis() as f64)
}
