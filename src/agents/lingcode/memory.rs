use crate::agents::install_status::hidden_home_parent;
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
        let home = self.core.agent_home();
        let claude_home = hidden_home_parent(home).join(".claude");
        Ok(collect_memory_paths(
            &[
                home.join("config"),
                home.join("history"),
                home.join("sessions"),
                claude_home.join("hooks"),
            ],
            &["lingcode".to_string(), "memory".to_string()],
        ))
    }
}
