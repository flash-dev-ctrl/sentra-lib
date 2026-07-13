use std::sync::Arc;

use crate::SentraResult;
use crate::interfaces::{ProviderData, Scanner};
use crate::risks::checkers::{CheckOutput, RiskChecker};

use super::utils::build_prompt_input;

const PROVIDER_SCANNER_ID: &str = "provider-scanner";

pub(crate) struct ProviderScanner {
    checker: Arc<RiskChecker>,
}

impl ProviderScanner {
    pub(crate) fn new(checker: Arc<RiskChecker>) -> Self {
        Self { checker }
    }
}

impl Scanner<ProviderData> for ProviderScanner {
    fn id(&self) -> &str {
        PROVIDER_SCANNER_ID
    }

    fn scan_asset<'a>(
        &'a self,
        asset: &'a ProviderData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = SentraResult<CheckOutput>> + Send + 'a>>
    {
        Box::pin(async move {
            let Some(base_url) = asset.base_url.as_deref().filter(|value| !value.is_empty()) else {
                return self.checker.scan(&[]).await;
            };
            let input = build_prompt_input(base_url, base_url);
            self.checker.scan(&[input]).await
        })
    }
}
