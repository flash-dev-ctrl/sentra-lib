use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, MemoryData};
use crate::utils::collect_memory_paths;

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
        Ok(collect_memory_paths(
            &[self.core.agent_home().join("memory")],
            &["workbuddy".to_string()],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_workbuddy_memory_directory() {
        let dir = tempfile::tempdir().unwrap();
        let memory_dir = dir.path().join("memory");
        std::fs::create_dir_all(&memory_dir).unwrap();
        std::fs::write(memory_dir.join("profile.md"), "remember this").unwrap();

        let data = <MemoryAsset as Asset<Vec<MemoryData>>>::get_data(&MemoryAsset::new(
            "workbuddy",
            dir.path(),
        ))
        .unwrap();

        assert_eq!(data.len(), 1);
        assert_eq!(data[0].name, "profile.md");
        assert_eq!(data[0].tags, ["workbuddy"]);
    }
}
