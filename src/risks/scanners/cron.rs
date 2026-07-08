use std::sync::Arc;

use super::utils::{build_content_input, path_content_inputs};
use crate::SentraResult;
use crate::interfaces::{CronData, Scanner};
use crate::risks::checkers::{CheckOutput, RiskChecker};

const CRON_SCANNER_ID: &str = "cron-scanner";

pub(crate) struct CronScanner {
    checker: Arc<RiskChecker>,
}

impl CronScanner {
    pub(crate) fn new(checker: Arc<RiskChecker>) -> Self {
        Self { checker }
    }
}

impl Scanner<CronData> for CronScanner {
    fn id(&self) -> &str {
        CRON_SCANNER_ID
    }

    fn scan_asset<'a>(
        &'a self,
        asset: &'a CronData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = SentraResult<CheckOutput>> + Send + 'a>>
    {
        Box::pin(async move {
            if let Some(home) = &asset.home
                && let Ok(inputs) = path_content_inputs(home, false)
            {
                return self.checker.scan(&inputs).await;
            }

            if !asset.prompt.is_empty() {
                let source = format!("cron:{}:prompt", asset.id);
                let input = build_content_input(
                    std::path::Path::new(&source),
                    asset.prompt.clone(),
                    asset.prompt.as_bytes(),
                );
                return self.checker.scan(&[input]).await;
            }

            self.checker.scan(&[]).await
        })
    }
}
