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
        for extension_dir in super::extension_dirs(self.core.agent_home()) {
            let manifest_path = extension_dir.join("package.json");
            let Some(manifest) = read_json_file(&manifest_path)? else {
                continue;
            };
            if !super::is_agent_manifest(&manifest) {
                continue;
            }
            collect_crons(&manifest, &manifest_path, &mut results);
        }
        for manifest_path in super::agent_plugin_manifests(self.core.agent_home()) {
            let Some(manifest) = read_json_file(&manifest_path)? else {
                continue;
            };
            if super::is_agent_manifest(&manifest) {
                collect_crons(&manifest, &manifest_path, &mut results);
            }
        }
        Ok(results)
    }
}

fn collect_crons(value: &serde_json::Value, home: &std::path::Path, results: &mut Vec<CronData>) {
    let Some(contributes) = value.get("contributes") else {
        return;
    };
    for key in ["scheduledTasks", "cron", "hooks"] {
        let Some(raw) = contributes.get(key) else {
            continue;
        };
        for item in raw.as_array().into_iter().flatten() {
            let Some(id) = string_field(item, "id").or_else(|| string_field(item, "name")) else {
                continue;
            };
            let schedule = string_field(item, "cron").or_else(|| string_field(item, "schedule"));
            results.push(CronData {
                id: id.clone(),
                name: string_field(item, "name").unwrap_or(id),
                prompt: string_field(item, "prompt").unwrap_or_default(),
                enabled: item
                    .get("enabled")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
                home: Some(home.to_path_buf()),
                cron_type: schedule.as_ref().map(|_| CronType::Cron),
                schedule,
                cwds: Vec::new(),
                created_at: None,
                updated_at: None,
                files: Vec::new(),
            });
        }
    }
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
