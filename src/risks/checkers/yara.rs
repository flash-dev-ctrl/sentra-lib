use crate::SentraResult;
use crate::i18n::{category_zh, messages, severity_zh};
use crate::interfaces::{
    CheckInput, CheckResult, Checker, FileCategory, FileExtType, Finding, RiskCategory,
    RiskSeverity,
};
use crate::risks::checkers::unified::{ok, skipped};
use crate::risks::rule_store::CompiledYaraRule;
use crate::utils::context::line_window_context;
use std::collections::HashMap;

pub const YARA_CHECKER_ID: &str = "yara-checker";

pub struct YaraChecker {
    rules: Vec<CompiledYaraRule>,
}

impl YaraChecker {
    pub fn new(rules: Vec<CompiledYaraRule>) -> Self {
        Self { rules }
    }
}

impl Checker for YaraChecker {
    fn id(&self) -> &str {
        YARA_CHECKER_ID
    }

    fn name(&self) -> &str {
        messages::YARA_CHECKER_NAME
    }

    fn description(&self) -> &str {
        messages::YARA_CHECKER_DESCRIPTION
    }

    fn categories(&self) -> &[FileCategory] {
        &[FileCategory::Prompt, FileCategory::Script]
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
            if !self
                .categories()
                .contains(&content.file_cat.unwrap_or(FileCategory::Unknown))
            {
                return Ok(skipped(self.id(), "unsupported category"));
            }
            if self.rules.is_empty() {
                return Ok(skipped(self.id(), "no YARA rules configured"));
            }
            let mut findings = Vec::new();
            for rule in &self.rules {
                let mut scanner = yara_x::Scanner::new(&rule.rules);
                let results = scanner.scan(content.content.as_bytes()).map_err(|err| {
                    crate::SentraError::Message(format!(
                        "failed to scan with YARA rule {}: {err}",
                        rule.source.display()
                    ))
                })?;
                for matched_rule in results.matching_rules() {
                    let meta = collect_meta(&matched_rule);
                    let file_cat = content.file_cat.unwrap_or(FileCategory::Unknown);
                    if !matches_file_meta(&meta, file_cat, content.file_ext) {
                        continue;
                    }
                    let tags = matched_rule
                        .tags()
                        .map(|tag| tag.identifier().to_string())
                        .collect::<Vec<_>>();
                    findings.push(yara_finding(
                        self.id(),
                        content.source.as_str(),
                        &content.content,
                        matched_rule.identifier(),
                        &meta,
                        &tags,
                        first_match(&matched_rule),
                    ));
                }
            }
            Ok(ok(self.id(), findings))
        })
    }
}

fn yara_finding(
    checker: &str,
    source: &str,
    content: &str,
    rule_name: &str,
    meta: &HashMap<String, String>,
    tags: &[String],
    first_match: Option<YaraMatch>,
) -> Finding {
    let severity = determine_severity(meta, tags);
    let category = determine_category(meta, tags);
    let title = meta_string(meta, "title").unwrap_or(rule_name);
    let description = meta_string(meta, "description")
        .map(str::to_string)
        .unwrap_or_else(|| fallback_description(rule_name, meta, tags));
    let remediation = meta_string(meta, "remediation")
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "Review the matched content for potential security issues. YARA rule \"{rule_name}\" flagged this content."
            )
        });

    let mut finding = Finding::new(
        format!("{checker}:{rule_name}"),
        checker,
        severity,
        category,
        source,
        title,
        description,
        remediation,
    );
    finding.severity_zh = Some(
        meta_string(meta, "severity_zh")
            .unwrap_or_else(|| severity_zh(severity))
            .to_string(),
    );
    finding.category_zh = Some(
        meta_string(meta, "category_zh")
            .unwrap_or_else(|| category_zh(category))
            .to_string(),
    );
    finding.title_zh = Some(meta_string(meta, "title_zh").unwrap_or(title).to_string());
    finding.description_zh = Some(
        meta_string(meta, "description_zh")
            .map(str::to_string)
            .unwrap_or_else(|| fallback_description_zh(rule_name, meta, tags)),
    );
    finding.remediation_zh = Some(
        meta_string(meta, "remediation_zh")
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "检查匹配内容是否存在潜在安全问题。YARA 规则 \"{rule_name}\" 标记了该内容。"
                )
            }),
    );
    if let Some(matched) = first_match {
        finding.location.line = line_number_by_offset(content, matched.offset);
        finding.evidence = Some(matched.data);
        finding.context = line_context(content, finding.location.line);
    } else {
        finding.location.line = 1;
        finding.evidence = Some(rule_name.to_string());
        finding.context = line_context(content, 1);
    }
    finding
}

#[derive(Debug, Clone)]
struct YaraMatch {
    offset: usize,
    data: String,
}

fn collect_meta(rule: &yara_x::Rule<'_, '_>) -> HashMap<String, String> {
    rule.metadata()
        .map(|(key, value)| (key.to_string(), meta_value_to_string(value)))
        .collect()
}

fn meta_value_to_string(value: yara_x::MetaValue<'_>) -> String {
    match value {
        yara_x::MetaValue::Integer(value) => value.to_string(),
        yara_x::MetaValue::Float(value) => value.to_string(),
        yara_x::MetaValue::Bool(value) => value.to_string(),
        yara_x::MetaValue::String(value) => value.to_string(),
        yara_x::MetaValue::Bytes(value) => String::from_utf8_lossy(value.as_ref()).to_string(),
    }
}

fn first_match(rule: &yara_x::Rule<'_, '_>) -> Option<YaraMatch> {
    rule.patterns()
        .flat_map(|pattern| {
            pattern.matches().map(|matched| YaraMatch {
                offset: matched.range().start,
                data: String::from_utf8_lossy(matched.data()).to_string(),
            })
        })
        .min_by_key(|matched| matched.offset)
}

fn matches_file_meta(
    meta: &HashMap<String, String>,
    file_cat: FileCategory,
    file_ext: Option<FileExtType>,
) -> bool {
    if let Some(file_categories) = meta_string(meta, "file_categories") {
        let input_category = file_category_name(file_cat);
        return file_categories
            .split(',')
            .map(|category| category.trim().to_ascii_lowercase())
            .any(|category| category == input_category);
    }

    if let (Some(file_type), Some(file_ext)) = (meta_string(meta, "file_type"), file_ext) {
        return file_type.eq_ignore_ascii_case(file_ext_type_name(file_ext));
    }

    true
}

fn determine_severity(meta: &HashMap<String, String>, tags: &[String]) -> RiskSeverity {
    if let Some(value) = meta_string(meta, "severity").and_then(parse_severity) {
        return value;
    }
    if has_tag(tags, "malware") {
        return RiskSeverity::Critical;
    }
    if has_tag(tags, "suspicious") {
        return RiskSeverity::High;
    }
    RiskSeverity::Medium
}

fn determine_category(meta: &HashMap<String, String>, tags: &[String]) -> RiskCategory {
    if let Some(value) = meta_string(meta, "category").and_then(parse_category) {
        return value;
    }
    if has_tag(tags, "network") {
        return RiskCategory::NetworkAccess;
    }
    if has_tag(tags, "file") {
        return RiskCategory::FileSystem;
    }
    if has_tag(tags, "credential") {
        return RiskCategory::CredentialExposure;
    }
    if has_tag(tags, "injection") {
        return RiskCategory::PromptInjection;
    }
    RiskCategory::SupplyChain
}

fn parse_severity(value: &str) -> Option<RiskSeverity> {
    match normalize(value).as_str() {
        "CRITICAL" => Some(RiskSeverity::Critical),
        "HIGH" => Some(RiskSeverity::High),
        "MEDIUM" => Some(RiskSeverity::Medium),
        "LOW" => Some(RiskSeverity::Low),
        "INFO" | "INFORMATIONAL" => Some(RiskSeverity::Info),
        _ => None,
    }
}

fn parse_category(value: &str) -> Option<RiskCategory> {
    match normalize(value).as_str() {
        "PROMPTINJECTION" => Some(RiskCategory::PromptInjection),
        "DATAEXFILTRATION" => Some(RiskCategory::DataExfiltration),
        "PRIVILEGEESCALATION" => Some(RiskCategory::PrivilegeEscalation),
        "NETWORKACCESS" => Some(RiskCategory::NetworkAccess),
        "FILESYSTEM" => Some(RiskCategory::FileSystem),
        "CREDENTIALEXPOSURE" => Some(RiskCategory::CredentialExposure),
        "SUPPLYCHAIN" => Some(RiskCategory::SupplyChain),
        "MISCONFIGURATION" => Some(RiskCategory::Misconfiguration),
        "POLYGLOT" => Some(RiskCategory::Polyglot),
        "MALICIOUSEXECUTION" => Some(RiskCategory::MaliciousExecution),
        "CRYPTOMINING" => Some(RiskCategory::CryptoMining),
        "WEBSHELL" => Some(RiskCategory::WebShell),
        "HACKTOOL" => Some(RiskCategory::HackTool),
        "EXPLOIT" => Some(RiskCategory::Exploit),
        _ => None,
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

fn meta_string<'a>(meta: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    meta.get(key)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn has_tag(tags: &[String], expected: &str) -> bool {
    tags.iter().any(|tag| tag.eq_ignore_ascii_case(expected))
}

fn fallback_description(
    rule_name: &str,
    meta: &HashMap<String, String>,
    tags: &[String],
) -> String {
    let tags = format_tags(tags);
    let meta_info = format_meta(meta);
    format!(
        "Pattern detected by YARA rule \"{rule_name}\"{}{}",
        if tags.is_empty() {
            String::new()
        } else {
            format!(" {tags}")
        },
        if meta_info.is_empty() {
            String::new()
        } else {
            format!(" ({meta_info})")
        }
    )
}

fn fallback_description_zh(
    rule_name: &str,
    meta: &HashMap<String, String>,
    tags: &[String],
) -> String {
    let tags = format_tags(tags);
    let meta_info = format_meta(meta);
    format!(
        "YARA 规则 \"{rule_name}\" 检测到匹配模式{}{}",
        if tags.is_empty() {
            String::new()
        } else {
            format!(" {tags}")
        },
        if meta_info.is_empty() {
            String::new()
        } else {
            format!("（{meta_info}）")
        }
    )
}

fn format_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        String::new()
    } else {
        format!("[{}]", tags.join(", "))
    }
}

fn format_meta(meta: &HashMap<String, String>) -> String {
    let mut parts = Vec::new();
    if let Some(severity) = meta_string(meta, "severity") {
        parts.push(format!("severity: {severity}"));
    }
    if let Some(description) = meta_string(meta, "description") {
        parts.push(description.to_string());
    }
    parts.join(", ")
}

fn line_number_by_offset(content: &str, offset: usize) -> usize {
    1 + content.as_bytes()[..offset.min(content.len())]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
}

fn line_context(content: &str, line: usize) -> Option<String> {
    line_window_context(content, line, 0, 0, Some(200))
}
fn file_category_name(category: FileCategory) -> &'static str {
    match category {
        FileCategory::Unknown => "unknown",
        FileCategory::Prompt => "prompt",
        FileCategory::Script => "script",
        FileCategory::Exe => "exe",
        FileCategory::Binary => "binary",
        FileCategory::Mcp => "mcp",
    }
}

fn file_ext_type_name(file_ext: FileExtType) -> &'static str {
    match file_ext {
        FileExtType::Unknown => "unknown",
        FileExtType::Md => "md",
        FileExtType::Json => "json",
        FileExtType::Yaml => "yaml",
        FileExtType::Js => "js",
        FileExtType::Ts => "ts",
        FileExtType::Py => "py",
        FileExtType::Sh => "sh",
        FileExtType::Ps1 => "ps1",
        FileExtType::Bat => "bat",
    }
}
