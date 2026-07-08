use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use super::RuleFileType;

pub(crate) fn detect_rule_file_type(path: &Path, content: &str) -> RuleFileType {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "yar" || ext == "yara" {
        return RuleFileType::Yara;
    }

    let hash_extensions: HashSet<&str> = [".txt", ".csv", ".json"].iter().cloned().collect();
    let file_ext = format!(".{ext}");
    if (name.starts_with("white") || name.starts_with("black"))
        && hash_extensions.contains(file_ext.as_str())
        && looks_like_hash_feed(content)
    {
        return RuleFileType::Hash;
    }

    if (ext == "txt" || ext == "csv" || ext.is_empty()) && looks_like_ti_feed(content) {
        return RuleFileType::Ti;
    }

    let trimmed = content.trim();
    if yara_rule_re().is_match(trimmed) {
        return RuleFileType::Yara;
    }
    if looks_like_ti_feed(trimmed) {
        return RuleFileType::Ti;
    }

    RuleFileType::Unknown
}

fn looks_like_ti_feed(content: &str) -> bool {
    let mut data_lines = 0;
    let mut ti_lines = 0;
    let ip_re = ip_line_re();
    let domain_re = domain_line_re();
    for line in content.lines().take(50) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        data_lines += 1;
        if ip_re.is_match(trimmed) || domain_re.is_match(trimmed) {
            ti_lines += 1;
        }
    }
    data_lines >= 3 && (ti_lines as f64 / data_lines as f64) >= 0.5
}

fn looks_like_hash_feed(content: &str) -> bool {
    let mut data_lines = 0;
    let mut hash_lines = 0;
    let re = hash_line_re();
    for line in content.lines().take(50) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        data_lines += 1;
        if re.is_match(trimmed) {
            hash_lines += 1;
        }
    }
    data_lines >= 1 && (hash_lines as f64 / data_lines as f64) >= 0.5
}

fn ip_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(?:\d{1,3}\.){3}\d{1,3}(?:[\s,/]|$)").unwrap())
}

fn domain_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^[a-zA-Z0-9](?:[a-zA-Z0-9.-]*[a-zA-Z0-9])?\.[a-zA-Z]{2,}$").unwrap()
    })
}

fn hash_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b[a-fA-F0-9]{32}\b|\b[a-fA-F0-9]{40}\b|\b[a-fA-F0-9]{64}\b").unwrap()
    })
}

fn yara_rule_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^rule\s+\w+").unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_yara_by_extension() {
        assert_eq!(
            detect_rule_file_type(Path::new("rules.yar"), "not a rule"),
            RuleFileType::Yara
        );
    }

    #[test]
    fn detects_yara_by_content() {
        assert_eq!(
            detect_rule_file_type(
                Path::new("rules.txt"),
                "rule foo {\n  strings:\n    $a = \"x\"\n}"
            ),
            RuleFileType::Yara
        );
    }

    #[test]
    fn detects_ti_feed() {
        let content = "1.2.3.4\nexample.com\n5.6.7.8\n";
        assert_eq!(
            detect_rule_file_type(Path::new("ti.txt"), content),
            RuleFileType::Ti
        );
    }

    #[test]
    fn detects_hash_feed_by_name() {
        let content = "d41d8cd98f00b204e9800998ecf8427e\n";
        assert_eq!(
            detect_rule_file_type(Path::new("black.md5.txt"), content),
            RuleFileType::Hash
        );
    }
}
