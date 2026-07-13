use crate::SentraResult;
use crate::agents::object::AssetCore;
use crate::interfaces::{Asset, AssetType, ErasedAsset, MetaData};
use crate::utils::dir_exists;

#[derive(Debug, Clone)]
pub(super) struct MetaAsset {
    pub(crate) core: AssetCore,
}

impl MetaAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl ErasedAsset for MetaAsset {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn asset_type(&self) -> AssetType {
        AssetType::Meta
    }

    fn agent_name(&self) -> &str {
        self.core.agent_name()
    }

    fn agent_home(&self) -> &std::path::Path {
        self.core.agent_home()
    }

    fn data(&self) -> SentraResult<serde_json::Value> {
        serde_json::to_value(<Self as Asset<Option<MetaData>>>::get_data(self)?)
            .map_err(|err| crate::SentraError::Message(err.to_string()))
    }

    fn data_async<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = SentraResult<serde_json::Value>> + 'a>>
    {
        Box::pin(async move {
            serde_json::to_value(<Self as Asset<Option<MetaData>>>::get_data_async(self).await?)
                .map_err(|err| crate::SentraError::Message(err.to_string()))
        })
    }
}

impl Asset<Option<MetaData>> for MetaAsset {
    fn get_data(&self) -> SentraResult<Option<MetaData>> {
        let home = self.core.agent_home();
        if !dir_exists(home) {
            return Ok(None);
        }
        let agent_name = self.core.agent_name().to_string();
        Ok(Some(MetaData {
            id: Some(agent_name.clone()),
            name: agent_name,
            description: Some(
                "Cloud-based AI coding agent by OpenAI that runs sandboxed tasks and writes, tests, and fixes code autonomously."
                    .to_string(),
            ),
            version: None,
            author: Some("OpenAI".to_string()),
            home: Some(home.to_path_buf()),
            created_at: None,
            updated_at: None,
        }))
    }
}
