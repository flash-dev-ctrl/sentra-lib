use std::sync::Arc;

use super::utils::path_content_inputs;
use crate::SentraResult;
use crate::interfaces::{Scanner, SkillData};
use crate::risks::checkers::{CheckOutput, RiskChecker};

const SKILL_SCANNER_ID: &str = "skill-scanner";

pub(crate) struct SkillScanner {
    checker: Arc<RiskChecker>,
}

impl SkillScanner {
    pub(crate) fn new(checker: Arc<RiskChecker>) -> Self {
        Self { checker }
    }
}

impl Scanner<SkillData> for SkillScanner {
    fn id(&self) -> &str {
        SKILL_SCANNER_ID
    }

    fn scan_asset<'a>(
        &'a self,
        asset: &'a SkillData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = SentraResult<CheckOutput>> + Send + 'a>>
    {
        Box::pin(async move {
            if let Some(home) = &asset.home {
                let inputs = path_content_inputs(home, false)?;
                self.checker.scan(&inputs).await
            } else {
                self.checker.scan(&[]).await
            }
        })
    }
}
