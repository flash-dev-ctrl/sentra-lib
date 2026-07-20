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
        let mut results = Vec::new();
        for file in collect_memory_paths(&[self.core.agent_home().join("hooks")], &[]) {
            results.push(CronData {
                id: file.path.to_string_lossy().to_string(),
                name: file.name,
                prompt: read_text_file(&file.path)?.unwrap_or_default(),
                enabled: true,
                home: Some(file.path),
                ..CronData::default()
            });
        }
        Ok(results)
    }
}
