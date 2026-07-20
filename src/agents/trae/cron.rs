use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
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
        let mut files = vec![self.core.agent_home().join("hooks.json")];
        if let Some(path) = crate::agents::trae::workspace_path(".trae/hooks.json") {
            files.push(path);
        }
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
        Ok(results)
    }
}
