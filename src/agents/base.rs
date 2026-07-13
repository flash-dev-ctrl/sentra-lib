use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::{
    discovery::get_agent_title,
    entries::{AgentAssetFactory, AgentEntry},
};
use crate::interfaces::{AssetType, ErasedAsset};

#[derive(Debug, Clone)]
pub struct Agent {
    name: String,
    home: PathBuf,
    title: String,
    asset_for_type: AgentAssetFactory,
}

impl Agent {
    pub(crate) fn new(entry: &AgentEntry, home: impl Into<PathBuf>) -> Self {
        let name = entry.name.to_string();
        Self {
            title: entry
                .title
                .map(str::to_string)
                .unwrap_or_else(|| get_agent_title(&name)),
            name,
            home: home.into(),
            asset_for_type: entry.asset_for_type,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn get_assets(&self, asset_type: AssetType) -> SentraResult<Vec<Box<dyn ErasedAsset>>> {
        Ok((self.asset_for_type)(&self.name, &self.home, asset_type))
    }
}
