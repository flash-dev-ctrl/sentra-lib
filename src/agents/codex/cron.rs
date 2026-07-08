use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, CronData, CronType};
use crate::utils::{dir_exists, read_text_file};

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
        let automations_dir = self.core.agent_home().join("automations");
        if !dir_exists(&automations_dir) {
            return Ok(Vec::new());
        }
        let mut results = Vec::new();
        for entry in std::fs::read_dir(automations_dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
        {
            let entry_dir = entry.path();
            let Some(content) = read_text_file(entry_dir.join("automation.toml"))? else {
                continue;
            };
            let Ok(parsed) = toml::from_str::<toml::Value>(&content) else {
                continue;
            };
            let id = parsed
                .get("id")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| entry.file_name().to_string_lossy().to_string());
            let status = parsed.get("status").and_then(|value| value.as_str());
            let rrule = parsed
                .get("rrule")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let cwds = parsed
                .get("cwds")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            results.push(CronData {
                id: id.clone(),
                name: parsed
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or(&id)
                    .to_string(),
                prompt: parsed
                    .get("prompt")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                enabled: !matches!(status, Some("INACTIVE" | "DISABLED")),
                home: Some(entry_dir),
                cron_type: rrule.as_ref().map(|_| CronType::Rrule),
                schedule: rrule,
                cwds,
                created_at: None,
                updated_at: None,
                files: Vec::new(),
            });
        }
        Ok(results)
    }
}
