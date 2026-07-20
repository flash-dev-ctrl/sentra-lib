use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{Asset, AssetType, CronData};
use crate::utils::{collect_memory_paths, read_text_file};
use crate::SentraResult;

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
        cron_data(self.core.agent_home())
    }
}

fn cron_data(agent_home: &std::path::Path) -> SentraResult<Vec<CronData>> {
    let files = collect_memory_paths(
        &[agent_home.join("hooks.json"), agent_home.join("hooks")],
        &[],
    );
    let mut results = Vec::new();
    for file in files {
        let prompt = read_text_file(&file.path)?.unwrap_or_default();
        results.push(CronData {
            id: file.path.to_string_lossy().to_string(),
            name: file.name,
            prompt,
            enabled: true,
            home: Some(file.path),
            ..CronData::default()
        });
    }
    Ok(results)
}
