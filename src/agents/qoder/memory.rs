use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{Asset, AssetType, MemoryData};
use crate::utils::collect_memory_paths;
use crate::SentraResult;

#[derive(Debug, Clone)]
pub(super) struct MemoryAsset {
    core: AssetCore,
}

impl MemoryAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(MemoryAsset, AssetType::Memory, Vec<MemoryData>);

impl Asset<Vec<MemoryData>> for MemoryAsset {
    fn get_data(&self) -> SentraResult<Vec<MemoryData>> {
        let cwd = std::env::current_dir().unwrap_or_default();
        let project_home = cwd.join(format!(".{}", self.core.agent_name()));
        Ok(collect_memory_paths(
            &[
                self.core.agent_home().join("AGENTS.md"),
                self.core.agent_home().join("rules"),
                project_home.join("AGENTS.md"),
                project_home.join("rules"),
                cwd.join("AGENTS.md"),
            ],
            &["qoder".to_string()],
        ))
    }
}
