use std::path::PathBuf;

use crate::config::{sentra_hash_rule_dir, sentra_ti_rule_dir, sentra_yara_rule_dir};
use crate::{SentraError, SentraResult};

pub(crate) fn safe_name(name: &str) -> String {
    PathBuf::from(name)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| {
            n.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect()
        })
        .unwrap_or_else(|| "unnamed".to_string())
}

pub(crate) fn ensure_yara_name(name: &str) -> String {
    let safe = safe_name(name);
    let lower = safe.to_lowercase();
    if lower.ends_with(".yar") || lower.ends_with(".yara") {
        safe
    } else {
        format!("{safe}.yar")
    }
}

pub(crate) fn ensure_ti_name(name: &str) -> String {
    let safe = safe_name(name);
    let lower = safe.to_lowercase();
    if lower.ends_with(".txt") || lower.ends_with(".csv") {
        safe
    } else {
        format!("{safe}.txt")
    }
}

pub(crate) fn ensure_hash_name(name: &str) -> String {
    let safe = safe_name(name);
    if safe.to_lowercase().ends_with(".txt") {
        safe
    } else {
        format!("{safe}.txt")
    }
}

pub(crate) fn is_zip_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

pub(crate) fn default_yara_dir() -> Option<PathBuf> {
    home::home_dir().map(sentra_yara_rule_dir)
}

pub(crate) fn default_ti_dir() -> Option<PathBuf> {
    home::home_dir().map(sentra_ti_rule_dir)
}

pub(crate) fn default_hash_dir() -> Option<PathBuf> {
    home::home_dir().map(sentra_hash_rule_dir)
}

pub(crate) fn resolve_dir(
    configured: Option<PathBuf>,
    default: fn() -> Option<PathBuf>,
    kind: &str,
) -> SentraResult<PathBuf> {
    configured
        .or_else(default)
        .ok_or_else(|| SentraError::Message(format!("{kind} rule directory not configured")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_safe_name() {
        assert_eq!(safe_name("../../a/b rules.yar"), "b_rules.yar");
    }

    #[test]
    fn ensures_yara_extension() {
        assert_eq!(ensure_yara_name("rules"), "rules.yar");
        assert_eq!(ensure_yara_name("rules.yara"), "rules.yara");
    }
}
