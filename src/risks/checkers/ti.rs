use regex::Regex;
use serde_json::Value;
use std::sync::Mutex;

use crate::SentraResult;
use crate::i18n::{category_zh, messages, severity_zh};
use crate::interfaces::{
    CheckInput, CheckResult, Checker, FileCategory, Finding, RiskCategory, RiskSeverity,
};
use crate::risks::checkers::unified::{ok, skipped};
use crate::risks::types::{OnlineTiConfig, TiRuleDef};

pub const THREAT_INTEL_CHECKER_ID: &str = "threat-intel-checker";

const THREAT_INTEL_CATEGORIES: &[FileCategory] = &[FileCategory::Prompt, FileCategory::Script];
const SINKHOLE_IP: &str = "0.0.0.0";
const MAX_CONSECUTIVE_FAILURES: u8 = 3;
const THREATBOOK_API_URL: &str = "https://api.threatbook.cn/v3";
const CHAITIN_RIVERS_URL: &str = "https://ip-0.rivers.chaitin.cn";

pub struct ThreatIntelChecker {
    ip_pattern: Regex,
    domain_pattern: Regex,
    rules: TiRuleDef,
    online_ti: Option<OnlineTiConfig>,
    failures: Mutex<ProviderFailures>,
}

#[derive(Default)]
struct ProviderFailures {
    cloudflare: u8,
    threatbook: u8,
    chaitin: u8,
}

#[derive(Debug, Clone)]
struct OnlineQueryResult {
    blocked: bool,
    source: String,
}

impl ThreatIntelChecker {
    #[allow(clippy::new_without_default)]
    pub fn new(rules: TiRuleDef) -> Self {
        Self {
            ip_pattern: Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").unwrap(),
            domain_pattern: Regex::new(r"\b(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}\b").unwrap(),
            rules,
            online_ti: None,
            failures: Mutex::new(ProviderFailures::default()),
        }
    }

    pub fn new_with_online_ti(rules: TiRuleDef, online_ti: Option<OnlineTiConfig>) -> Self {
        let mut checker = Self::new(rules);
        checker.online_ti = online_ti;
        checker
    }
}

impl Checker for ThreatIntelChecker {
    fn id(&self) -> &str {
        THREAT_INTEL_CHECKER_ID
    }

    fn name(&self) -> &str {
        messages::THREAT_INTEL_CHECKER_NAME
    }

    fn description(&self) -> &str {
        messages::THREAT_INTEL_CHECKER_DESCRIPTION
    }

    fn categories(&self) -> &[FileCategory] {
        THREAT_INTEL_CATEGORIES
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
                let category = content.file_cat.unwrap_or(FileCategory::Unknown);
                return Ok(skipped(
                    self.id(),
                    &format!("unsupported category: {}", file_category_name(category)),
                ));
            }

            let mut findings = Vec::new();
            for (line_index, line) in content.content.lines().enumerate() {
                for mat in self.ip_pattern.find_iter(line) {
                    let ip = mat.as_str();
                    if is_private_ip(ip) {
                        continue;
                    }
                    let mut blocked = self.rules.malicious_ips.contains(ip);
                    let mut source = "offline".to_string();
                    if !blocked && let Some(result) = self.query_ip_online(ip).await {
                        blocked = result.blocked;
                        source = result.source;
                    }
                    if !blocked {
                        continue;
                    }
                    findings.push(network_finding(
                        self.id(),
                        &content.source,
                        ip,
                        IndicatorKind::Ip,
                        &source,
                        LineMatch {
                            index: line_index,
                            line: line_index + 1,
                            context: line,
                        },
                    ));
                }
                for mat in self.domain_pattern.find_iter(line) {
                    let domain = mat.as_str().to_ascii_lowercase();
                    if is_local_domain(&domain) {
                        continue;
                    }
                    let mut blocked = self.rules.malicious_domains.contains(&domain);
                    let mut source = "offline".to_string();
                    if !blocked && let Some(result) = self.query_domain_online(&domain).await {
                        blocked = result.blocked;
                        source = result.source;
                    }
                    if !blocked {
                        continue;
                    }
                    findings.push(network_finding(
                        self.id(),
                        &content.source,
                        &domain,
                        IndicatorKind::Domain,
                        &source,
                        LineMatch {
                            index: line_index,
                            line: line_index + 1,
                            context: line,
                        },
                    ));
                }
            }
            Ok(ok(self.id(), findings))
        })
    }
}

impl ThreatIntelChecker {
    async fn query_domain_online(&self, domain: &str) -> Option<OnlineQueryResult> {
        let config = self.online_ti.as_ref()?;
        if config.cloudflare_url.is_some()
            && !self.provider_circuit_open(Provider::Cloudflare)
            && let Some(result) = self.record_provider_result(
                Provider::Cloudflare,
                query_cloudflare_domain(config.clone(), domain.to_string()).await,
            )
            && result.blocked
        {
            return Some(result);
        }

        if config.threatbook_key.is_some()
            && !self.provider_circuit_open(Provider::ThreatBook)
            && let Some(result) = self.record_provider_result(
                Provider::ThreatBook,
                query_threatbook(config.clone(), domain.to_string()).await,
            )
            && result.blocked
        {
            return Some(result);
        }

        None
    }

    async fn query_ip_online(&self, ip: &str) -> Option<OnlineQueryResult> {
        let config = self.online_ti.as_ref()?;
        if config.threatbook_key.is_some()
            && !self.provider_circuit_open(Provider::ThreatBook)
            && let Some(result) = self.record_provider_result(
                Provider::ThreatBook,
                query_threatbook(config.clone(), ip.to_string()).await,
            )
            && result.blocked
        {
            return Some(result);
        }

        if config.chaitin_key.is_some()
            && !self.provider_circuit_open(Provider::Chaitin)
            && let Some(result) = self.record_provider_result(
                Provider::Chaitin,
                query_chaitin(config.clone(), ip.to_string()).await,
            )
            && result.blocked
        {
            return Some(result);
        }

        None
    }

    fn provider_circuit_open(&self, provider: Provider) -> bool {
        let failures = self.failures.lock().unwrap();
        provider.failures(&failures) >= MAX_CONSECUTIVE_FAILURES
    }

    fn record_provider_result(
        &self,
        provider: Provider,
        result: Result<OnlineQueryResult, ()>,
    ) -> Option<OnlineQueryResult> {
        let mut failures = self.failures.lock().unwrap();
        match result {
            Ok(result) => {
                *provider.failures_mut(&mut failures) = 0;
                Some(result)
            }
            Err(()) => {
                let count = provider.failures_mut(&mut failures);
                *count = count.saturating_add(1);
                None
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Provider {
    Cloudflare,
    ThreatBook,
    Chaitin,
}

impl Provider {
    fn failures(self, failures: &ProviderFailures) -> u8 {
        match self {
            Self::Cloudflare => failures.cloudflare,
            Self::ThreatBook => failures.threatbook,
            Self::Chaitin => failures.chaitin,
        }
    }

    fn failures_mut(self, failures: &mut ProviderFailures) -> &mut u8 {
        match self {
            Self::Cloudflare => &mut failures.cloudflare,
            Self::ThreatBook => &mut failures.threatbook,
            Self::Chaitin => &mut failures.chaitin,
        }
    }
}

async fn query_cloudflare_domain(
    config: OnlineTiConfig,
    domain: String,
) -> Result<OnlineQueryResult, ()> {
    tokio::task::spawn_blocking(move || {
        let base_url = config.cloudflare_url.as_deref().ok_or(())?;
        let url = format!(
            "{}?name={}&type=A",
            base_url.trim_end_matches('/'),
            url_encode(&domain)
        );
        let json = http_get_json(&url)?;
        let blocked = json
            .get("Answer")
            .and_then(Value::as_array)
            .map(|answers| {
                answers.iter().any(|answer| {
                    answer
                        .get("data")
                        .and_then(Value::as_str)
                        .is_some_and(|data| data == SINKHOLE_IP)
                })
            })
            .unwrap_or(false);
        Ok(OnlineQueryResult {
            blocked,
            source: "Cloudflare DNS".to_string(),
        })
    })
    .await
    .map_err(|_| ())?
}

async fn query_threatbook(
    config: OnlineTiConfig,
    resource: String,
) -> Result<OnlineQueryResult, ()> {
    tokio::task::spawn_blocking(move || {
        let api_key = config.threatbook_key.as_deref().ok_or(())?;
        let base_url = config
            .threatbook_url
            .as_deref()
            .unwrap_or(THREATBOOK_API_URL)
            .trim_end_matches('/');
        let url = format!(
            "{base_url}/scene/dns?apikey={}&resource={}",
            url_encode(api_key),
            url_encode(&resource)
        );
        let json = http_get_json(&url)?;
        if json.get("response_code").and_then(Value::as_i64) != Some(0) {
            return Ok(OnlineQueryResult {
                blocked: false,
                source: "ThreatBook".to_string(),
            });
        }
        let info = json
            .pointer(&format!("/data/ips/{resource}"))
            .or_else(|| json.pointer(&format!("/data/domains/{resource}")))
            .or_else(|| first_object_child(json.pointer("/data/ips")))
            .or_else(|| first_object_child(json.pointer("/data/domains")));
        let blocked = info
            .and_then(|value| value.get("is_malicious"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(OnlineQueryResult {
            blocked,
            source: "ThreatBook".to_string(),
        })
    })
    .await
    .map_err(|_| ())?
}

async fn query_chaitin(config: OnlineTiConfig, ip: String) -> Result<OnlineQueryResult, ()> {
    tokio::task::spawn_blocking(move || {
        let api_key = config.chaitin_key.as_deref().ok_or(())?;
        let base_url = config
            .chaitin_url
            .as_deref()
            .unwrap_or(CHAITIN_RIVERS_URL)
            .trim_end_matches('/');
        let url = format!(
            "{base_url}/api/share/s?sk={}&ip={}",
            url_encode(api_key),
            url_encode(&ip)
        );
        let json = http_get_json(&url)?;
        let blocked = json
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && json
                .pointer("/data/ip_info/status")
                .and_then(Value::as_str)
                .is_some_and(|status| status == "malicious");
        Ok(OnlineQueryResult {
            blocked,
            source: "Chaitin Rivers".to_string(),
        })
    })
    .await
    .map_err(|_| ())?
}

fn http_get_json(url: &str) -> Result<Value, ()> {
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|_| ())?;
    let body = response.into_string().map_err(|_| ())?;
    serde_json::from_str(&body).map_err(|_| ())
}

fn first_object_child(value: Option<&Value>) -> Option<&Value> {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.values().next())
}

fn url_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn is_private_ip(ip: &str) -> bool {
    ip.starts_with("10.")
        || ip.starts_with("192.168.")
        || ip.starts_with("127.")
        || ip.starts_with("0.")
        || (ip.starts_with("172.")
            && ip
                .split('.')
                .nth(1)
                .and_then(|part| part.parse::<u8>().ok())
                .map(|part| (16..=31).contains(&part))
                .unwrap_or(false))
}

fn is_local_domain(domain: &str) -> bool {
    ["localhost", "local", "internal", "lan", "intranet"]
        .iter()
        .any(|item| domain.contains(item))
}

#[derive(Clone, Copy)]
enum IndicatorKind {
    Ip,
    Domain,
}

struct LineMatch<'a> {
    index: usize,
    line: usize,
    context: &'a str,
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

fn network_finding(
    checker: &str,
    source: &str,
    item: &str,
    kind: IndicatorKind,
    ti_source: &str,
    line_match: LineMatch<'_>,
) -> Finding {
    let (id, title, title_zh, description, description_zh, remediation, remediation_zh) = match kind
    {
        IndicatorKind::Ip => (
            format!("ti-ip-{}-{item}", line_match.index),
            format!("Malicious IP detected: {item}"),
            format!("检测到恶意 IP：{item}"),
            format!("IP address {item} is flagged by threat intelligence ({ti_source})"),
            format!("IP 地址 {item} 被威胁情报标记（{ti_source}）"),
            "Review and remove the malicious IP reference",
            "检查并移除该恶意 IP 引用",
        ),
        IndicatorKind::Domain => (
            format!("ti-domain-{}-{item}", line_match.index),
            format!("Malicious domain detected: {item}"),
            format!("检测到恶意域名：{item}"),
            format!("Domain {item} is flagged by threat intelligence ({ti_source})"),
            format!("域名 {item} 被威胁情报标记（{ti_source}）"),
            "Review and remove the malicious domain reference",
            "检查并移除该恶意域名引用",
        ),
    };

    let trimmed_context = line_match.context.trim();
    let mut finding = Finding::new(
        id,
        checker,
        RiskSeverity::High,
        RiskCategory::NetworkAccess,
        source,
        title,
        description,
        remediation,
    );
    finding.severity_zh = Some(severity_zh(RiskSeverity::High).to_string());
    finding.category_zh = Some(category_zh(RiskCategory::NetworkAccess).to_string());
    finding.location.line = line_match.line;
    finding.title_zh = Some(title_zh);
    finding.description_zh = Some(description_zh);
    finding.evidence = Some(trimmed_context.chars().take(100).collect());
    finding.context = Some(trimmed_context.chars().take(200).collect());
    finding.remediation_zh = Some(remediation_zh.to_string());
    finding
}
