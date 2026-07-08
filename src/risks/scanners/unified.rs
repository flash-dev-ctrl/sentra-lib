use std::cmp::Reverse;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::SentraResult;
use crate::i18n::{category_zh, severity_zh};
use crate::interfaces::{
    CheckInput, CronData, Finding, MemoryData, ProviderData, RiskSeverity, Scanner, SkillData,
};
use crate::risks::checkers::{CheckError, RiskChecker};
use crate::risks::types::{RuleLoadSummary, RuleType, ScanOptions};

use super::cron::CronScanner;
use super::memory::MemoryScanner;
use super::provider::ProviderScanner;
use super::skill::SkillScanner;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanMetadata {
    pub scanner: String,
    pub scan_time: String,
    pub scan_duration_ms: u128,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanSummary {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub info: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanReport {
    pub metadata: ScanMetadata,
    pub summary: ScanSummary,
    pub findings: Vec<Finding>,
    pub errors: Vec<CheckError>,
}

pub enum RiskAsset<'a> {
    Skill(&'a SkillData),
    Cron(&'a CronData),
    Memory(&'a MemoryData),
    Provider(&'a ProviderData),
    CheckInput(CheckInput),
    Unsupported,
}

enum ScannerSelection<'a> {
    Skill(&'a SkillData),
    Cron(&'a CronData),
    Memory(&'a MemoryData),
    Provider(&'a ProviderData),
    CheckInput(CheckInput),
}

impl<'a> From<&'a SkillData> for RiskAsset<'a> {
    fn from(asset: &'a SkillData) -> Self {
        Self::Skill(asset)
    }
}

impl<'a> From<&'a CronData> for RiskAsset<'a> {
    fn from(asset: &'a CronData) -> Self {
        Self::Cron(asset)
    }
}

impl<'a> From<&'a MemoryData> for RiskAsset<'a> {
    fn from(asset: &'a MemoryData) -> Self {
        Self::Memory(asset)
    }
}

impl<'a> From<&'a ProviderData> for RiskAsset<'a> {
    fn from(asset: &'a ProviderData) -> Self {
        Self::Provider(asset)
    }
}

impl From<CheckInput> for RiskAsset<'_> {
    fn from(input: CheckInput) -> Self {
        Self::CheckInput(input)
    }
}

pub struct RiskScanner {
    checker: Arc<RiskChecker>,
    cron_scanner: CronScanner,
    memory_scanner: MemoryScanner,
    provider_scanner: ProviderScanner,
    skill_scanner: SkillScanner,
}

impl RiskScanner {
    pub fn new(options: ScanOptions) -> SentraResult<Self> {
        let checker = Arc::new(RiskChecker::new(options)?);
        Ok(Self::from_checker(checker))
    }

    pub fn load_rule(&mut self, rule_type: RuleType) -> SentraResult<RuleLoadSummary> {
        self.checker.load_rule(rule_type)
    }

    pub fn load_rules(&mut self) -> SentraResult<RuleLoadSummary> {
        self.checker.load_rules()
    }

    pub fn enabled_rule_types(&self) -> Vec<RuleType> {
        self.checker.enabled_rule_types()
    }

    pub fn concurrency(&self) -> usize {
        self.checker.concurrency()
    }

    fn from_checker(checker: Arc<RiskChecker>) -> Self {
        Self {
            cron_scanner: CronScanner::new(Arc::clone(&checker)),
            memory_scanner: MemoryScanner::new(Arc::clone(&checker)),
            provider_scanner: ProviderScanner::new(Arc::clone(&checker)),
            skill_scanner: SkillScanner::new(Arc::clone(&checker)),
            checker,
        }
    }

    pub async fn scan(&self, asset: RiskAsset<'_>) -> SentraResult<ScanReport> {
        let started_at = Utc::now();
        let started = Instant::now();
        let (scanner, output) = match select_scanner(asset) {
            Some(ScannerSelection::Skill(asset)) => {
                let scanner = self.skill_scanner.id().to_string();
                (scanner, self.skill_scanner.scan_asset(asset).await?)
            }
            Some(ScannerSelection::Cron(asset)) => {
                let scanner = self.cron_scanner.id().to_string();
                (scanner, self.cron_scanner.scan_asset(asset).await?)
            }
            Some(ScannerSelection::Memory(asset)) => {
                let scanner = self.memory_scanner.id().to_string();
                (scanner, self.memory_scanner.scan_asset(asset).await?)
            }
            Some(ScannerSelection::Provider(asset)) => {
                let scanner = self.provider_scanner.id().to_string();
                (scanner, self.provider_scanner.scan_asset(asset).await?)
            }
            Some(ScannerSelection::CheckInput(input)) => (
                "risk-scanner".to_string(),
                self.checker.scan(&[input]).await?,
            ),
            None => ("none".to_string(), Default::default()),
        };
        Ok(build_scan_report(
            &scanner,
            started_at.to_rfc3339(),
            started.elapsed().as_millis(),
            output.findings,
            output.errors,
        ))
    }
}

fn select_scanner(asset: RiskAsset<'_>) -> Option<ScannerSelection<'_>> {
    match asset {
        RiskAsset::Skill(asset) => Some(ScannerSelection::Skill(asset)),
        RiskAsset::Cron(asset) => Some(ScannerSelection::Cron(asset)),
        RiskAsset::Memory(asset) => Some(ScannerSelection::Memory(asset)),
        RiskAsset::Provider(asset) => Some(ScannerSelection::Provider(asset)),
        RiskAsset::CheckInput(input) => Some(ScannerSelection::CheckInput(input)),
        RiskAsset::Unsupported => None,
    }
}

fn build_scan_report(
    scanner: &str,
    scan_time: String,
    scan_duration_ms: u128,
    mut findings: Vec<Finding>,
    errors: Vec<CheckError>,
) -> ScanReport {
    for finding in &mut findings {
        finding.severity_zh = finding
            .severity_zh
            .take()
            .or_else(|| Some(severity_zh(finding.severity).to_string()));
        finding.category_zh = finding
            .category_zh
            .take()
            .or_else(|| Some(category_zh(finding.category).to_string()));
        finding
            .title_zh
            .get_or_insert_with(|| finding.title.clone());
        finding
            .description_zh
            .get_or_insert_with(|| finding.description.clone());
        finding
            .remediation_zh
            .get_or_insert_with(|| finding.remediation.clone());
    }

    findings.sort_by_key(|finding| Reverse(finding.severity));

    let summary = findings
        .iter()
        .fold(ScanSummary::default(), |mut summary, finding| {
            match finding.severity {
                RiskSeverity::Critical => summary.critical += 1,
                RiskSeverity::High => summary.high += 1,
                RiskSeverity::Medium => summary.medium += 1,
                RiskSeverity::Low => summary.low += 1,
                RiskSeverity::Info => summary.info += 1,
            }
            summary
        });

    ScanReport {
        metadata: ScanMetadata {
            scanner: scanner.to_string(),
            scan_time,
            scan_duration_ms,
        },
        summary,
        findings,
        errors,
    }
}
