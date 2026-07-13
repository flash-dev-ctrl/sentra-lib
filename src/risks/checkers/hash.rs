use crate::SentraResult;
use crate::i18n::{category_zh, messages, severity_zh};
use crate::interfaces::{
    CheckInput, CheckResult, CheckStatus, Checker, ContentInput, FileCategory, Finding,
    RiskCategory, RiskSeverity,
};
use crate::risks::checkers::unified::{ok, skipped};
use crate::risks::types::HashRuleDef;
use crate::utils::{Hashes, compute_content_hashes};

pub const HASH_CHECKER_ID: &str = "hash-checker";
pub const HASH_WHITELIST_REASON: &str = "hash whitelisted";

pub struct HashChecker {
    rules: HashRuleDef,
}

impl HashChecker {
    pub fn new(rules: HashRuleDef) -> Self {
        Self { rules }
    }
}

impl Checker for HashChecker {
    fn id(&self) -> &str {
        HASH_CHECKER_ID
    }

    fn name(&self) -> &str {
        messages::HASH_CHECKER_NAME
    }

    fn description(&self) -> &str {
        messages::HASH_CHECKER_DESCRIPTION
    }

    fn categories(&self) -> &[FileCategory] {
        &[]
    }

    fn check<'a>(
        &'a self,
        input: &'a CheckInput,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = SentraResult<CheckResult>> + Send + 'a>>
    {
        Box::pin(async move {
            let CheckInput::Content(content) = input else {
                return Ok(skipped(self.id(), "not a content input"));
            };
            let hashes = read_input_hashes(content);
            let values = [&hashes.md5, &hashes.sha1, &hashes.sha256];

            if values
                .iter()
                .any(|hash| self.rules.whitelist.contains(*hash))
            {
                return Ok(CheckResult {
                    checker: self.id().to_string(),
                    status: CheckStatus::Ok,
                    reason: Some(HASH_WHITELIST_REASON.to_string()),
                    findings: Vec::new(),
                });
            }

            let Some(blacklisted) = values
                .iter()
                .find(|hash| self.rules.blacklist.contains(hash.as_str()))
            else {
                return Ok(ok(self.id(), Vec::new()));
            };

            let mut finding = Finding::new(
                format!(
                    "{}-{}",
                    self.id(),
                    &hashes.sha256[..hashes.sha256.len().min(12)]
                ),
                self.id(),
                RiskSeverity::Critical,
                RiskCategory::SupplyChain,
                &content.source,
                "Blacklisted file hash detected",
                format!("File content hash is present in the local blacklist: {blacklisted}"),
                "Remove this file or replace it with a trusted version.",
            );
            finding.severity_zh = Some(severity_zh(RiskSeverity::Critical).to_string());
            finding.category_zh = Some(category_zh(RiskCategory::SupplyChain).to_string());
            finding.location.line = 1;
            finding.title_zh = Some("检测到黑名单文件 Hash".to_string());
            finding.description_zh = Some(format!("文件内容 Hash 命中本地黑名单：{blacklisted}"));
            finding.remediation_zh = Some("移除该文件，或替换为可信版本。".to_string());
            finding.evidence = Some((*blacklisted).clone());
            finding.context = Some(format!(
                "md5={} sha1={} sha256={}",
                hashes.md5, hashes.sha1, hashes.sha256
            ));
            Ok(ok(self.id(), vec![finding]))
        })
    }
}

fn read_input_hashes(input: &ContentInput) -> Hashes {
    let mut hashes = input
        .other
        .get("hashes")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_else(|| compute_content_hashes(input.content.as_bytes()));
    hashes.md5 = hashes.md5.to_ascii_lowercase();
    hashes.sha1 = hashes.sha1.to_ascii_lowercase();
    hashes.sha256 = hashes.sha256.to_ascii_lowercase();
    hashes
}
