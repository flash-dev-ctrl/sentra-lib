use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, ProcessData};
use crate::utils::mask_secret;

pub(crate) type ProcessMatcher = fn(&ProcessInfo<'_>) -> bool;

#[derive(Debug)]
pub(crate) struct ProcessInfo<'a> {
    pub(crate) name: &'a str,
    pub(crate) cmdline: &'a [String],
    pub(crate) path: Option<&'a Path>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessAsset {
    core: AssetCore,
    matcher: ProcessMatcher,
}

impl ProcessAsset {
    pub(crate) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<PathBuf>,
        matcher: ProcessMatcher,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
            matcher,
        }
    }
}

impl_erased_asset!(ProcessAsset, AssetType::Process, Vec<ProcessData>);

impl Asset<Vec<ProcessData>> for ProcessAsset {
    fn get_data(&self) -> SentraResult<Vec<ProcessData>> {
        Ok(process_data(self.matcher))
    }
}

pub(crate) fn process_data(matcher: ProcessMatcher) -> Vec<ProcessData> {
    let mut system = sysinfo::System::new_all();
    system.refresh_all();

    let mut results = system
        .processes()
        .values()
        .filter_map(|process| process_record(process, matcher))
        .collect::<Vec<_>>();
    results.sort_by_key(|process| process.pid);
    results
}

fn process_record(process: &sysinfo::Process, matcher: ProcessMatcher) -> Option<ProcessData> {
    let name = os_to_string(process.name());
    let cmdline = process
        .cmd()
        .iter()
        .map(|arg| os_to_string(arg))
        .collect::<Vec<_>>();
    let path = process.exe().map(Path::to_path_buf);
    let info = ProcessInfo {
        name: &name,
        cmdline: &cmdline,
        path: path.as_deref(),
    };

    if !matcher(&info) {
        return None;
    }

    Some(ProcessData {
        pid: process.pid().as_u32(),
        name,
        cmdline,
        started_at: process.start_time(),
        run_time_seconds: process.run_time(),
        path,
        env: sanitized_env(process.environ()),
    })
}

pub(crate) fn matches_binary_names(process: &ProcessInfo<'_>, binary_names: &[&str]) -> bool {
    if is_binary_name(process.name, binary_names) {
        return true;
    }
    process
        .cmdline
        .first()
        .map(|command| is_binary_name(command_basename(command), binary_names))
        .unwrap_or(false)
}

pub(crate) fn is_binary_name(value: &str, binary_names: &[&str]) -> bool {
    let value = normalized_binary_name(value);
    binary_names
        .iter()
        .any(|binary| value == normalized_binary_name(binary))
}

pub(crate) fn path_has_component(path: &Path, components: &[&str]) -> bool {
    path.components().any(|component| {
        let Some(component) = component.as_os_str().to_str() else {
            return false;
        };
        let component = normalized_binary_name(component);
        components
            .iter()
            .any(|expected| component == normalized_binary_name(expected))
    })
}

fn command_basename(command: &str) -> &str {
    command.rsplit(['/', '\\']).next().unwrap_or(command)
}

fn normalized_binary_name(value: &str) -> String {
    value
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase()
}

fn sanitized_env(entries: &[OsString]) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for entry in entries {
        let entry = entry.to_string_lossy();
        let Some((key, value)) = entry.split_once('=') else {
            continue;
        };
        if key.is_empty() {
            continue;
        }
        let value = if is_sensitive_env_key(key) {
            mask_secret(Some(value)).unwrap_or_default()
        } else {
            value.to_string()
        };
        env.insert(key.to_string(), value);
    }
    env
}

fn is_sensitive_env_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "key",
        "token",
        "secret",
        "password",
        "credential",
        "auth",
        "bearer",
        "session",
        "cookie",
        "private",
    ]
    .iter()
    .any(|marker| key.contains(marker))
}

fn os_to_string(value: &OsStr) -> String {
    value.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_process_by_binary_name() {
        assert_matches_binary_names("codex", &[], None, &["codex", "codex.exe"]);
        assert_matches_binary_names("Codex.exe", &[], None, &["codex", "codex.exe"]);
    }

    #[test]
    fn matches_process_by_first_command() {
        assert_matches_binary_names(
            "node",
            &[r"C:\Users\me\AppData\Local\Programs\OpenAI\Codex\codex.exe"],
            None,
            &["codex", "codex.exe"],
        );
        assert_matches_binary_names(
            "node",
            &["/usr/local/bin/codex"],
            None,
            &["codex", "codex.exe"],
        );
    }

    #[test]
    fn does_not_match_binary_name_as_substring() {
        assert_not_matches_binary_names("my-codex-helper", &[], None, &["codex", "codex.exe"]);
        assert_not_matches_binary_names(
            "node",
            &["/usr/local/bin/my-codex-helper"],
            None,
            &["codex", "codex.exe"],
        );
    }

    #[test]
    fn matches_path_component_case_insensitively() {
        let chatgpt_path = Path::new("Users")
            .join("me")
            .join("AppData")
            .join("Local")
            .join("Programs")
            .join("OpenAI")
            .join("ChatGPT")
            .join("ChatGPT.exe");
        let temp_path = Path::new("temp").join("ChatGPT.exe");

        assert!(path_has_component(&chatgpt_path, &["openai", "chatgpt"]));
        assert!(!path_has_component(&temp_path, &["openai"]));
    }

    #[test]
    fn sanitizes_sensitive_environment_values() {
        let env = sanitized_env(&[
            OsString::from("OPENAI_API_KEY=sk-1234567890"),
            OsString::from("PATH=/usr/bin"),
        ]);

        assert_eq!(env.get("PATH").map(String::as_str), Some("/usr/bin"));
        let api_key = env.get("OPENAI_API_KEY").unwrap();
        assert_ne!(api_key, "sk-1234567890");
        assert!(api_key.contains("****"));
    }

    fn assert_matches_binary_names(
        name: &str,
        cmdline: &[&str],
        path: Option<&Path>,
        binary_names: &[&str],
    ) {
        let cmdline = cmdline
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let process = ProcessInfo {
            name,
            cmdline: &cmdline,
            path,
        };
        assert!(matches_binary_names(&process, binary_names));
    }

    fn assert_not_matches_binary_names(
        name: &str,
        cmdline: &[&str],
        path: Option<&Path>,
        binary_names: &[&str],
    ) {
        let cmdline = cmdline
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let process = ProcessInfo {
            name,
            cmdline: &cmdline,
            path,
        };
        assert!(!matches_binary_names(&process, binary_names));
    }
}
