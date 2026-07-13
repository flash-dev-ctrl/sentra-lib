use std::fs;

use sentra_lib::interfaces::{
    CheckInput, FileCategory, FileExtType, Finding, RiskCategory, RiskSeverity,
};
use sentra_lib::risks::checkers::RiskChecker;
use sentra_lib::risks::types::{CheckerConfig, YaraClassification, YaraRule};
use sentra_lib::risks::{RuleDirectoryConfig, ScanOptions};

fn run_yara(rule: &str, content: &str, cat: FileCategory, ext: FileExtType) -> Vec<Finding> {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("demo.yar"), rule).unwrap();

    let input = CheckInput::content("demo.md", content).with_file_meta(cat, ext);
    let checker = RiskChecker::new(ScanOptions {
        checker: Some(CheckerConfig {
            enable_hash: Some(false),
            enable_yara: Some(true),
            enable_llm: Some(false),
            enable_local_ti: Some(false),
            enable_online_ti: Some(false),
        }),
        rules: Some(RuleDirectoryConfig {
            yara: Some(dir.path().to_path_buf()),
            ..Default::default()
        }),
        ..ScanOptions::default()
    })
    .unwrap();
    let output = block_on(checker.scan(&[input])).unwrap();
    output.findings
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn yara_rule_schema_shape_matches_single_rule_metadata() {
    let rule: YaraRule = serde_json::from_str(
        r#"{
            "name": "prompt_injection_generic",
            "tags": ["prompt", "suspicious"],
            "meta": {
                "author": "Cisco",
                "title": "Prompt Injection Detection",
                "title_zh": "提示词注入检测",
                "description": "Detects prompt strings",
                "description_zh": "检测提示词字符串",
                "remediation": "Sanitize prompts.",
                "remediation_zh": "清理提示词。",
                "classification": "harmful",
                "threat_type": "PROMPT INJECTION",
                "confidence": "0.85",
                "reference": "https://example.com",
                "severity": "HIGH",
                "severity_zh": "高危",
                "category": "PROMPT_INJECTION",
                "category_zh": "提示词注入",
                "file_type": "md",
                "file_categories": ["prompt"]
            }
        }"#,
    )
    .unwrap();

    assert_eq!(rule.name, "prompt_injection_generic");
    assert_eq!(rule.tags, vec!["prompt", "suspicious"]);
    assert_eq!(rule.meta.classification, YaraClassification::Harmful);
    assert_eq!(rule.meta.severity, RiskSeverity::High);
    assert_eq!(rule.meta.category, RiskCategory::PromptInjection);
    assert_eq!(rule.meta.file_categories, vec![FileCategory::Prompt]);
}

#[test]
fn maps_yara_meta_fields_to_finding() {
    let findings = run_yara(
        r#"
rule MetaMappedRule {
    meta:
        severity = "LOW"
        severity_zh = "低"
        category = "CREDENTIAL_EXPOSURE"
        category_zh = "凭据"
        title = "Credential marker"
        title_zh = "凭据标记"
        description = "Credential-like content was found"
        description_zh = "发现类似凭据的内容"
        remediation = "Rotate the exposed value"
        remediation_zh = "轮换暴露的值"
    strings:
        $secret = "api-token"
    condition:
        $secret
}
"#,
        "before api-token after",
        FileCategory::Prompt,
        FileExtType::Md,
    );

    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.severity, RiskSeverity::Low);
    assert_eq!(finding.severity_zh.as_deref(), Some("低"));
    assert_eq!(finding.category, RiskCategory::CredentialExposure);
    assert_eq!(finding.category_zh.as_deref(), Some("凭据"));
    assert_eq!(finding.title, "Credential marker");
    assert_eq!(finding.title_zh.as_deref(), Some("凭据标记"));
    assert_eq!(finding.description, "Credential-like content was found");
    assert_eq!(
        finding.description_zh.as_deref(),
        Some("发现类似凭据的内容")
    );
    assert_eq!(finding.remediation, "Rotate the exposed value");
    assert_eq!(finding.remediation_zh.as_deref(), Some("轮换暴露的值"));
}

#[test]
fn falls_back_from_yara_tags_for_severity_and_category() {
    let findings = run_yara(
        r#"
rule MalwareTagged : malware {
    strings: $a = "malware-marker"
    condition: $a
}

rule SuspiciousTagged : suspicious {
    strings: $a = "suspicious-marker"
    condition: $a
}

rule NetworkTagged : network {
    strings: $a = "network-marker"
    condition: $a
}

rule FileTagged : file {
    strings: $a = "file-marker"
    condition: $a
}

rule CredentialTagged : credential {
    strings: $a = "credential-marker"
    condition: $a
}

rule InjectionTagged : injection {
    strings: $a = "injection-marker"
    condition: $a
}
"#,
        "malware-marker suspicious-marker network-marker file-marker credential-marker injection-marker",
        FileCategory::Script,
        FileExtType::Js,
    );

    assert_eq!(findings.len(), 6);
    let by_title = |rule: &str| {
        findings
            .iter()
            .find(|finding| finding.title == rule)
            .unwrap_or_else(|| panic!("missing finding for {rule}"))
    };

    assert_eq!(by_title("MalwareTagged").severity, RiskSeverity::Critical);
    assert_eq!(by_title("SuspiciousTagged").severity, RiskSeverity::High);
    assert_eq!(
        by_title("NetworkTagged").category,
        RiskCategory::NetworkAccess
    );
    assert_eq!(by_title("FileTagged").category, RiskCategory::FileSystem);
    assert_eq!(
        by_title("CredentialTagged").category,
        RiskCategory::CredentialExposure
    );
    assert_eq!(
        by_title("InjectionTagged").category,
        RiskCategory::PromptInjection
    );
}

#[test]
fn skips_rules_when_file_categories_or_file_type_do_not_match() {
    let findings = run_yara(
        r#"
rule PromptOnly {
    meta:
        file_categories = "prompt"
    strings:
        $a = "shared-marker"
    condition:
        $a
}

rule PythonOnly {
    meta:
        file_type = "py"
    strings:
        $a = "shared-marker"
    condition:
        $a
}

rule JavaScriptOnly {
    meta:
        file_type = "js"
    strings:
        $a = "shared-marker"
    condition:
        $a
}
"#,
        "shared-marker",
        FileCategory::Script,
        FileExtType::Js,
    );

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].title, "JavaScriptOnly");
}

#[test]
fn uses_first_matching_string_for_evidence_line_and_context() {
    let findings = run_yara(
        r#"
rule EvidenceRule {
    strings:
        $first = "first-hit"
        $second = "second-hit"
    condition:
        any of them
}
"#,
        "line one\nline two has first-hit here\nline three has second-hit",
        FileCategory::Prompt,
        FileExtType::Md,
    );

    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.evidence.as_deref(), Some("first-hit"));
    assert_eq!(finding.location.line, 2);
    assert_eq!(
        finding.context.as_deref(),
        Some("line two has first-hit here")
    );
}
