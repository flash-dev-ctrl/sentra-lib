use std::fs;

use sentra_lib::interfaces::{CheckInput, FileCategory, FileExtType, RiskCategory, RiskSeverity};
use sentra_lib::risks::checkers::RiskChecker;
use sentra_lib::risks::types::CheckerConfig;
use sentra_lib::risks::{RuleDirectoryConfig, ScanOptions};
use serde_json::json;
use tempfile::TempDir;

fn hash_checker(rules: RuleDirectoryConfig) -> (TempDir, RiskChecker) {
    let dir = rules.hash.clone().unwrap();
    let temp = TempDir::new_in(dir.parent().unwrap()).unwrap();
    let checker = RiskChecker::new(ScanOptions {
        checker: Some(CheckerConfig {
            enable_hash: Some(true),
            enable_yara: Some(false),
            enable_llm: Some(false),
            enable_local_ti: Some(false),
            enable_online_ti: Some(false),
        }),
        rules: Some(rules),
        ..ScanOptions::default()
    })
    .unwrap();
    (temp, checker)
}

fn store_with_hash_rules(blacklist: &str, whitelist: &str) -> (TempDir, RuleDirectoryConfig) {
    let dir = tempfile::tempdir().unwrap();
    if !blacklist.is_empty() {
        fs::write(dir.path().join("black.txt"), blacklist).unwrap();
    }
    if !whitelist.is_empty() {
        fs::write(dir.path().join("white.txt"), whitelist).unwrap();
    }

    (
        dir,
        RuleDirectoryConfig {
            hash: None,
            ..RuleDirectoryConfig::default()
        },
    )
}

fn hash_rules(dir: &TempDir) -> RuleDirectoryConfig {
    RuleDirectoryConfig {
        hash: Some(dir.path().to_path_buf()),
        ..RuleDirectoryConfig::default()
    }
}

fn ti_checker(feed: &str) -> (TempDir, RiskChecker) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("feed.txt"), feed).unwrap();
    let checker = RiskChecker::new(ScanOptions {
        checker: Some(CheckerConfig {
            enable_hash: Some(false),
            enable_yara: Some(false),
            enable_llm: Some(false),
            enable_local_ti: Some(true),
            enable_online_ti: Some(false),
        }),
        rules: Some(RuleDirectoryConfig {
            ti: Some(dir.path().to_path_buf()),
            ..RuleDirectoryConfig::default()
        }),
        ..ScanOptions::default()
    })
    .unwrap();
    (dir, checker)
}

fn content_with_hashes(md5: &str, sha1: &str, sha256: &str) -> CheckInput {
    let CheckInput::Content(mut content) = CheckInput::content("sample.bin", "ignored") else {
        unreachable!("CheckInput::content returns Content");
    };
    content.other.insert(
        "hashes".to_string(),
        json!({
            "md5": md5,
            "sha1": sha1,
            "sha256": sha256,
        }),
    );
    CheckInput::Content(content)
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn hash_checker_normalizes_input_hashes_for_blacklist_and_whitelist() {
    let md5 = "0123456789abcdef0123456789abcdef";
    let sha1 = "0123456789abcdef0123456789abcdef01234567";
    let sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let (white_dir, _) = store_with_hash_rules("", sha1);
    let (_keep, checker) = hash_checker(hash_rules(&white_dir));
    let output =
        block_on(checker.scan(&[content_with_hashes(md5, &sha1.to_ascii_uppercase(), sha256)]))
            .unwrap();
    assert!(output.findings.is_empty());

    let (black_dir, _) = store_with_hash_rules(md5, "");
    let (_keep, checker) = hash_checker(hash_rules(&black_dir));
    let output =
        block_on(checker.scan(&[content_with_hashes(&md5.to_ascii_uppercase(), sha1, sha256)]))
            .unwrap();
    assert_eq!(output.findings.len(), 1);
    assert_eq!(output.findings[0].evidence.as_deref(), Some(md5));
}

#[test]
fn hash_checker_blacklist_finding_matches_offline_core_fields() {
    let md5 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let sha1 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let sha256 = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    let (dir, _) = store_with_hash_rules(sha256, "");
    let (_keep, checker) = hash_checker(hash_rules(&dir));

    let output =
        block_on(checker.scan(&[content_with_hashes(md5, sha1, &sha256.to_ascii_uppercase())]))
            .unwrap();

    assert_eq!(output.findings.len(), 1);
    let finding = &output.findings[0];
    assert_eq!(finding.id, format!("hash-checker-{}", &sha256[..12]));
    assert_eq!(finding.checker, "hash-checker");
    assert_eq!(finding.severity, RiskSeverity::Critical);
    assert_eq!(finding.severity_zh.as_deref(), Some("严重"));
    assert_eq!(finding.category, RiskCategory::SupplyChain);
    assert_eq!(finding.category_zh.as_deref(), Some("供应链风险"));
    assert_eq!(finding.file, "sample.bin");
    assert_eq!(finding.location.line, 1);
    assert_eq!(finding.title, "Blacklisted file hash detected");
    assert_eq!(finding.title_zh.as_deref(), Some("检测到黑名单文件 Hash"));
    assert_eq!(
        finding.description,
        format!("File content hash is present in the local blacklist: {sha256}")
    );
    assert_eq!(
        finding.description_zh.as_deref(),
        Some(format!("文件内容 Hash 命中本地黑名单：{sha256}").as_str())
    );
    assert_eq!(finding.evidence.as_deref(), Some(sha256));
    assert_eq!(
        finding.context.as_deref(),
        Some(format!("md5={md5} sha1={sha1} sha256={sha256}").as_str())
    );
    assert_eq!(
        finding.remediation,
        "Remove this file or replace it with a trusted version."
    );
    assert_eq!(
        finding.remediation_zh.as_deref(),
        Some("移除该文件，或替换为可信版本。")
    );
}

#[test]
fn threat_intel_offline_ip_and_domain_findings_match_core_fields() {
    let (_dir, checker) = ti_checker("203.0.113.9\nbad.example.com\n");
    let line1 = format!("connect to 203.0.113.9 with {}", "a".repeat(220));
    let line2 = format!(
        "fetch https://BAD.EXAMPLE.COM/path with {}",
        "b".repeat(220)
    );
    let input = CheckInput::content("agent.ts", format!("{line1}\n{line2}"))
        .with_file_meta(FileCategory::Script, FileExtType::Ts);

    let output = block_on(checker.scan(&[input])).unwrap();

    assert_eq!(output.findings.len(), 2);

    let ip = &output.findings[0];
    assert_eq!(ip.id, "ti-ip-0-203.0.113.9");
    assert_eq!(ip.severity, RiskSeverity::High);
    assert_eq!(ip.severity_zh.as_deref(), Some("高危"));
    assert_eq!(ip.category, RiskCategory::NetworkAccess);
    assert_eq!(ip.category_zh.as_deref(), Some("网络访问"));
    assert_eq!(ip.location.line, 1);
    assert_eq!(ip.title, "Malicious IP detected: 203.0.113.9");
    assert_eq!(ip.title_zh.as_deref(), Some("检测到恶意 IP：203.0.113.9"));
    assert_eq!(
        ip.description,
        "IP address 203.0.113.9 is flagged by threat intelligence (offline)"
    );
    assert_eq!(
        ip.description_zh.as_deref(),
        Some("IP 地址 203.0.113.9 被威胁情报标记（offline）")
    );
    assert_eq!(ip.evidence.as_deref(), Some(line1[..100].trim_end()));
    assert_eq!(ip.context.as_deref(), Some(line1[..200].trim_end()));
    assert_eq!(
        ip.remediation,
        "Review and remove the malicious IP reference"
    );
    assert_eq!(
        ip.remediation_zh.as_deref(),
        Some("检查并移除该恶意 IP 引用")
    );

    let domain = &output.findings[1];
    assert_eq!(domain.id, "ti-domain-1-bad.example.com");
    assert_eq!(domain.location.line, 2);
    assert_eq!(domain.title, "Malicious domain detected: bad.example.com");
    assert_eq!(
        domain.title_zh.as_deref(),
        Some("检测到恶意域名：bad.example.com")
    );
    assert_eq!(
        domain.description,
        "Domain bad.example.com is flagged by threat intelligence (offline)"
    );
    assert_eq!(
        domain.description_zh.as_deref(),
        Some("域名 bad.example.com 被威胁情报标记（offline）")
    );
    assert_eq!(domain.evidence.as_deref(), Some(line2[..100].trim_end()));
    assert_eq!(domain.context.as_deref(), Some(line2[..200].trim_end()));
    assert_eq!(
        domain.remediation,
        "Review and remove the malicious domain reference"
    );
    assert_eq!(
        domain.remediation_zh.as_deref(),
        Some("检查并移除该恶意域名引用")
    );
}

#[test]
fn threat_intel_feed_extracts_indicators_from_whitespace_separated_lines() {
    let (_dir, checker) = ti_checker("95.217.197.180 51pool.online\n");
    let input = CheckInput::content("SKILL.md", "connect to 95.217.197.180")
        .with_file_meta(FileCategory::Prompt, FileExtType::Md);

    let output = block_on(checker.scan(&[input])).unwrap();

    assert_eq!(output.findings.len(), 1);
    assert_eq!(
        output.findings[0].title,
        "Malicious IP detected: 95.217.197.180"
    );
}

#[test]
fn threat_intel_rule_loading_ignores_scores_and_descriptions() {
    let (_dir, checker) = ti_checker(
        "152.32.132.28\t9\n1.117.61.9,Possible Cobaltstrike C2 IP\nbad.example.com description\n",
    );

    let summary = checker.load_rules().unwrap();

    assert_eq!(summary.ti_ips, 2);
    assert_eq!(summary.ti_domains, 1);
}

#[test]
fn threat_intel_filters_private_ips_and_local_domains_but_matches_domain_case_insensitively() {
    let (_dir, checker) = ti_checker(
        "10.1.2.3\n192.168.1.1\n172.20.1.1\n127.0.0.1\n0.0.0.0\napi.localhost\ninternal.example.com\nevil.example.com\n",
    );
    let input = CheckInput::content(
        "prompt.md",
        "10.1.2.3 192.168.1.1 172.20.1.1 127.0.0.1 0.0.0.0 api.localhost internal.example.com EVIL.EXAMPLE.COM",
    )
    .with_file_meta(FileCategory::Prompt, FileExtType::Md);

    let output = block_on(checker.scan(&[input])).unwrap();

    assert_eq!(output.findings.len(), 1);
    assert_eq!(output.findings[0].id, "ti-domain-0-evil.example.com");
}
