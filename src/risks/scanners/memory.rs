use std::sync::Arc;

use super::utils::path_content_inputs;
use crate::SentraResult;
use crate::interfaces::{MemoryData, Scanner};
use crate::risks::checkers::{CheckOutput, RiskChecker};

const MEMORY_SCANNER_ID: &str = "memory-scanner";

pub(crate) struct MemoryScanner {
    checker: Arc<RiskChecker>,
}

impl MemoryScanner {
    pub(crate) fn new(checker: Arc<RiskChecker>) -> Self {
        Self { checker }
    }
}

impl Scanner<MemoryData> for MemoryScanner {
    fn id(&self) -> &str {
        MEMORY_SCANNER_ID
    }

    fn scan_asset<'a>(
        &'a self,
        asset: &'a MemoryData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = SentraResult<CheckOutput>> + Send + 'a>>
    {
        Box::pin(async move {
            if !asset.path.exists() {
                return Ok(CheckOutput::default());
            }
            let inputs = path_content_inputs(&asset.path, true)?;
            self.checker.scan(&inputs).await
        })
    }
}
