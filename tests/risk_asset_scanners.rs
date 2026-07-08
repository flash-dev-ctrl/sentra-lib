use std::fs;

use sentra_lib::interfaces::{CronData, MemoryData, ProviderData};
use sentra_lib::risks::{RiskAsset, RiskScanner, RuleDirectoryConfig, RuleType, ScanOptions};

const MARKER_RULE: &str = r#"
rule ScannerMarker {
    strings:
        $marker = "scanner-risk-marker"
    condition:
        $marker
}
"#;

#[test]
fn unified_scanner_dispatches_cron_asset_to_cron_scanner() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = write_rule_dir(dir.path());
    let scanner = scanner_with_yara(&rules_dir);
    let asset = CronData {
        id: "cron-demo".to_string(),
        name: "cron-demo".to_string(),
        prompt: "run scanner-risk-marker".to_string(),
        enabled: true,
        home: Some(dir.path().join("missing-cron")),
        ..CronData::default()
    };

    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();

    assert_eq!(report.metadata.scanner, "cron-scanner");
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].file, "cron:cron-demo:prompt");
}

#[test]
fn unified_scanner_dispatches_memory_asset_to_memory_scanner() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = write_rule_dir(dir.path());
    let memory_path = dir.path().join("memory.md");
    fs::write(&memory_path, "remember scanner-risk-marker").unwrap();
    let scanner = scanner_with_yara(&rules_dir);
    let asset = MemoryData {
        name: "memory.md".to_string(),
        path: memory_path.clone(),
        ..MemoryData::default()
    };

    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();

    assert_eq!(report.metadata.scanner, "memory-scanner");
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].file,
        memory_path.to_string_lossy().to_string()
    );
}

#[test]
fn memory_scanner_skips_missing_memory_files() {
    let dir = tempfile::tempdir().unwrap();
    let scanner = RiskScanner::new(ScanOptions::default()).unwrap();
    let asset = MemoryData {
        name: ".codex-global-state.json".to_string(),
        path: dir.path().join(".codex").join(".codex-global-state.json"),
        ..MemoryData::default()
    };

    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();

    assert_eq!(report.metadata.scanner, "memory-scanner");
    assert!(report.findings.is_empty());
    assert!(report.errors.is_empty());
}

#[test]
fn unified_scanner_dispatches_provider_asset_to_provider_scanner() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = write_rule_dir(dir.path());
    let scanner = scanner_with_yara(&rules_dir);
    let asset = ProviderData {
        name: "demo".to_string(),
        base_url: Some("https://scanner-risk-marker.example.com/v1".to_string()),
        enabled: true,
        ..ProviderData::default()
    };

    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();

    assert_eq!(report.metadata.scanner, "provider-scanner");
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].file,
        "https://scanner-risk-marker.example.com/v1"
    );
}

#[test]
fn provider_scanner_returns_empty_report_without_base_url() {
    let scanner = RiskScanner::new(ScanOptions::default()).unwrap();
    let asset = ProviderData {
        name: "demo".to_string(),
        base_url: None,
        ..ProviderData::default()
    };

    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();

    assert_eq!(report.metadata.scanner, "provider-scanner");
    assert!(report.findings.is_empty());
    assert!(report.errors.is_empty());
}

#[test]
fn risk_scanner_can_load_rules_explicitly_before_scanning() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = write_rule_dir(dir.path());
    let mut scanner = RiskScanner::new(ScanOptions {
        rules: Some(RuleDirectoryConfig {
            yara: Some(rules_dir),
            ..Default::default()
        }),
        ..ScanOptions::default()
    })
    .unwrap();

    let summary = scanner.load_rule(RuleType::Yara).unwrap();

    assert_eq!(summary.yara, 1);
    let asset = ProviderData {
        name: "demo".to_string(),
        base_url: Some("https://scanner-risk-marker.example.com/v1".to_string()),
        enabled: true,
        ..ProviderData::default()
    };
    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();
    assert_eq!(report.findings.len(), 1);
}

#[test]
fn risk_scanner_loads_rules_one_type_at_a_time() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = write_rule_dir(dir.path());
    let mut scanner = RiskScanner::new(ScanOptions {
        rules: Some(RuleDirectoryConfig {
            yara: Some(rules_dir),
            ..Default::default()
        }),
        ..ScanOptions::default()
    })
    .unwrap();

    let summary = scanner.load_rule(RuleType::Yara).unwrap();

    assert_eq!(summary.yara, 1);
    assert_eq!(summary.ti_ips, 0);
    assert_eq!(summary.hash_blacklist, 0);
}

fn write_rule_dir(root: &std::path::Path) -> std::path::PathBuf {
    let rules_dir = root.join("rules");
    fs::create_dir_all(&rules_dir).unwrap();
    fs::write(rules_dir.join("marker.yar"), MARKER_RULE).unwrap();
    rules_dir
}

fn scanner_with_yara(rules_dir: &std::path::Path) -> RiskScanner {
    RiskScanner::new(ScanOptions {
        rules: Some(RuleDirectoryConfig {
            yara: Some(rules_dir.to_path_buf()),
            ..Default::default()
        }),
        ..ScanOptions::default()
    })
    .unwrap()
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
