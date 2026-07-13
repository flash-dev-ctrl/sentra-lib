use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, MemoryData};
use crate::utils::collect_memory_paths;

#[derive(Debug, Clone)]
pub(super) struct MemoryAsset {
    pub(crate) core: AssetCore,
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
        Ok(memory_data(self.core.agent_home()))
    }
}

fn memory_data(agent_home: &std::path::Path) -> Vec<MemoryData> {
    collect_memory_paths(
        &[
            agent_home.join("memories"),
            agent_home.join("hermes-agent").join("AGENTS.md"),
            agent_home.join("SOUL.md"),
        ],
        &["hermes".to_string(), "memory".to_string()],
    )
}
