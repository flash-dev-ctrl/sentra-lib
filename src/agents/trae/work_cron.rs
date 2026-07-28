use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::agents::trae::{scheduled_task_cloud, scheduled_task_db, surface};
use crate::interfaces::{Asset, AssetType, CronData};
use crate::utils::read_text_file;

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
        let state_home = surface::state_home(self.core.agent_name(), self.core.agent_home());
        let files = [
            self.core.agent_home().join("hooks.json"),
            state_home.join("work").join("hooks.json"),
        ];
        let mut results = Vec::new();
        for path in files {
            let Some(content) = read_text_file(&path)? else {
                continue;
            };
            results.push(CronData {
                id: path.display().to_string(),
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| "hooks.json".to_string()),
                prompt: content,
                enabled: true,
                home: path.parent().map(std::path::Path::to_path_buf),
                ..CronData::default()
            });
        }
        results.extend(scheduled_task_db::cron_data(surface::work_data_roots(
            self.core.agent_name(),
            self.core.agent_home(),
        ))?);
        results.extend(scheduled_task_cloud::cron_data(
            self.core.agent_name(),
            self.core.agent_home(),
        ));
        results = dedup_crons(results);
        Ok(results)
    }
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
