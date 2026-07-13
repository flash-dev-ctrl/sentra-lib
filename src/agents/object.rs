use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct AssetCore {
    agent_name: String,
    agent_home: PathBuf,
}

impl AssetCore {
    pub(crate) fn new(agent_name: impl Into<String>, agent_home: impl Into<PathBuf>) -> Self {
        Self {
            agent_name: agent_name.into(),
            agent_home: agent_home.into(),
        }
    }

    pub(crate) fn agent_name(&self) -> &str {
        &self.agent_name
    }

    pub(crate) fn agent_home(&self) -> &Path {
        &self.agent_home
    }
}

macro_rules! impl_erased_asset {
    ($ty:ty, $asset_type:expr, $data:ty) => {
        impl crate::interfaces::ErasedAsset for $ty {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn asset_type(&self) -> crate::interfaces::AssetType {
                $asset_type
            }

            fn agent_name(&self) -> &str {
                self.core.agent_name()
            }

            fn agent_home(&self) -> &std::path::Path {
                self.core.agent_home()
            }

            fn data(&self) -> crate::SentraResult<serde_json::Value> {
                serde_json::to_value(<$ty as crate::interfaces::Asset<$data>>::get_data(self)?)
                    .map_err(|err| crate::SentraError::Message(err.to_string()))
            }

            fn data_async<'a>(
                &'a self,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = crate::SentraResult<serde_json::Value>> + 'a>,
            > {
                Box::pin(async move {
                    serde_json::to_value(
                        <$ty as crate::interfaces::Asset<$data>>::get_data_async(self).await?,
                    )
                    .map_err(|err| crate::SentraError::Message(err.to_string()))
                })
            }

            fn runtime_data(&self) -> crate::SentraResult<serde_json::Value> {
                serde_json::to_value(<$ty as crate::interfaces::Asset<$data>>::get_runtime_data(
                    self,
                )?)
                .map_err(|err| crate::SentraError::Message(err.to_string()))
            }

            fn runtime_data_async<'a>(
                &'a self,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = crate::SentraResult<serde_json::Value>> + 'a>,
            > {
                Box::pin(async move {
                    serde_json::to_value(
                        <$ty as crate::interfaces::Asset<$data>>::get_runtime_data_async(self)
                            .await?,
                    )
                    .map_err(|err| crate::SentraError::Message(err.to_string()))
                })
            }
        }
    };
    ($ty:ty, $asset_type:expr, $data:ty, $item:ty) => {
        impl crate::interfaces::ErasedAsset for $ty {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn asset_type(&self) -> crate::interfaces::AssetType {
                $asset_type
            }

            fn agent_name(&self) -> &str {
                self.core.agent_name()
            }

            fn agent_home(&self) -> &std::path::Path {
                self.core.agent_home()
            }

            fn data(&self) -> crate::SentraResult<serde_json::Value> {
                serde_json::to_value(<$ty as crate::interfaces::Asset<$data, $item>>::get_data(
                    self,
                )?)
                .map_err(|err| crate::SentraError::Message(err.to_string()))
            }

            fn data_async<'a>(
                &'a self,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = crate::SentraResult<serde_json::Value>> + 'a>,
            > {
                Box::pin(async move {
                    serde_json::to_value(
                        <$ty as crate::interfaces::Asset<$data, $item>>::get_data_async(self)
                            .await?,
                    )
                    .map_err(|err| crate::SentraError::Message(err.to_string()))
                })
            }

            fn runtime_data(&self) -> crate::SentraResult<serde_json::Value> {
                serde_json::to_value(
                    <$ty as crate::interfaces::Asset<$data, $item>>::get_runtime_data(self)?,
                )
                .map_err(|err| crate::SentraError::Message(err.to_string()))
            }

            fn runtime_data_async<'a>(
                &'a self,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = crate::SentraResult<serde_json::Value>> + 'a>,
            > {
                Box::pin(async move {
                    serde_json::to_value(
                        <$ty as crate::interfaces::Asset<$data, $item>>::get_runtime_data_async(
                            self,
                        )
                        .await?,
                    )
                    .map_err(|err| crate::SentraError::Message(err.to_string()))
                })
            }

            fn set_skill_data(
                &self,
                value: crate::interfaces::SkillData,
            ) -> crate::SentraResult<crate::interfaces::AssetMutationResult> {
                <Self as crate::interfaces::Asset<$data, $item>>::set_data(self, value)
            }

            fn del_skill_data(
                &self,
                item: &crate::interfaces::SkillData,
            ) -> crate::SentraResult<crate::interfaces::AssetMutationResult> {
                <Self as crate::interfaces::Asset<$data, $item>>::del_data(self, item)
            }
        }
    };
    ($ty:ty, $asset_type:expr, $data:ty, $item:ty, provider) => {
        impl crate::interfaces::ErasedAsset for $ty {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn asset_type(&self) -> crate::interfaces::AssetType {
                $asset_type
            }

            fn agent_name(&self) -> &str {
                self.core.agent_name()
            }

            fn agent_home(&self) -> &std::path::Path {
                self.core.agent_home()
            }

            fn data(&self) -> crate::SentraResult<serde_json::Value> {
                serde_json::to_value(<$ty as crate::interfaces::Asset<$data, $item>>::get_data(
                    self,
                )?)
                .map_err(|err| crate::SentraError::Message(err.to_string()))
            }

            fn data_async<'a>(
                &'a self,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = crate::SentraResult<serde_json::Value>> + 'a>,
            > {
                Box::pin(async move {
                    serde_json::to_value(
                        <$ty as crate::interfaces::Asset<$data, $item>>::get_data_async(self)
                            .await?,
                    )
                    .map_err(|err| crate::SentraError::Message(err.to_string()))
                })
            }

            fn runtime_data(&self) -> crate::SentraResult<serde_json::Value> {
                serde_json::to_value(
                    <$ty as crate::interfaces::Asset<$data, $item>>::get_runtime_data(self)?,
                )
                .map_err(|err| crate::SentraError::Message(err.to_string()))
            }

            fn runtime_data_async<'a>(
                &'a self,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = crate::SentraResult<serde_json::Value>> + 'a>,
            > {
                Box::pin(async move {
                    serde_json::to_value(
                        <$ty as crate::interfaces::Asset<$data, $item>>::get_runtime_data_async(
                            self,
                        )
                        .await?,
                    )
                    .map_err(|err| crate::SentraError::Message(err.to_string()))
                })
            }

            fn provider_requests(
                &self,
                model: &str,
            ) -> Vec<crate::interfaces::ProviderProbeRequest> {
                self.get_request(model)
            }

            fn set_provider_data(
                &self,
                value: crate::interfaces::ProviderData,
            ) -> crate::SentraResult<crate::interfaces::AssetMutationResult> {
                <Self as crate::interfaces::Asset<$data, $item>>::set_data(self, value)
            }

            fn del_provider_data(
                &self,
                item: &crate::interfaces::ProviderData,
            ) -> crate::SentraResult<crate::interfaces::AssetMutationResult> {
                <Self as crate::interfaces::Asset<$data, $item>>::del_data(self, item)
            }
        }
    };
}

pub(crate) use impl_erased_asset;
