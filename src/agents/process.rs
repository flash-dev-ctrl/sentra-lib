use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, ProcessData};
use crate::utils::{sanitize_command_args, sanitize_env_value};

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
        cmdline: sanitize_command_args(&cmdline),
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

pub(crate) fn process_has_ide_extension(process: &ProcessInfo<'_>, extension_id: &str) -> bool {
    process
        .path
        .is_some_and(|path| value_has_ide_extension(path.to_string_lossy().as_ref(), extension_id))
        || process
            .cmdline
            .first()
            .is_some_and(|command| value_has_ide_extension(command, extension_id))
}

fn value_has_ide_extension(value: &str, extension_id: &str) -> bool {
    let extension_id = extension_id.to_ascii_lowercase();
    let components = value
        .trim_matches(['"', '\''])
        .split(['/', '\\'])
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    components.windows(2).any(|pair| {
        pair[0] == "extensions"
            && (pair[1] == extension_id
                || pair[1]
                    .strip_prefix(&extension_id)
                    .is_some_and(|suffix| suffix.starts_with('-')))
    })
}

pub(crate) fn cmdline_has_path_components(process: &ProcessInfo<'_>, components: &[&str]) -> bool {
    !components.is_empty()
        && process.cmdline.iter().any(|arg| {
            let path_components = arg
                .trim_matches(['"', '\''])
                .split(['/', '\\'])
                .map(normalized_binary_name)
                .collect::<Vec<_>>();
            let expected = components
                .iter()
                .map(|component| normalized_binary_name(component))
                .collect::<Vec<_>>();
            path_components
                .windows(expected.len())
                .any(|window| window == expected.as_slice())
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
        let value = sanitize_env_value(key, value);
        env.insert(key.to_string(), value);
    }
    env
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
    fn matches_versioned_ide_extension_paths_exactly() {
        let path = Path::new("Users")
            .join("me")
            .join(".devin")
            .join("extensions")
            .join("openai.chatgpt-1.2.3-win32-x64")
            .join("bin")
            .join("codex.exe");
        let cmdline = vec![
            r#"C:\Users\me\.vscode\extensions\Anthropic.claude-code-2.0\claude.exe"#.to_string(),
        ];
        let process = ProcessInfo {
            name: "codex.exe",
            cmdline: &cmdline,
            path: Some(&path),
        };

        assert!(process_has_ide_extension(&process, "openai.chatgpt"));
        assert!(process_has_ide_extension(&process, "anthropic.claude-code"));
        assert!(!process_has_ide_extension(&process, "openai.chat"));
    }

    #[test]
    fn matches_all_cmdline_path_components_exactly() {
        let cmdline = vec![
            "node.exe".to_string(),
            r#"C:\Users\me\node_modules\@scope\agent\cli.js"#.to_string(),
        ];
        let process = ProcessInfo {
            name: "node.exe",
            cmdline: &cmdline,
            path: None,
        };

        assert!(cmdline_has_path_components(&process, &["@scope", "agent"]));
        assert!(!cmdline_has_path_components(
            &process,
            &["@scope", "agent-helper"]
        ));
        assert!(!cmdline_has_path_components(
            &process,
            &["node_modules", "agent"]
        ));
    }

    #[test]
    fn sanitizes_sensitive_environment_values() {
        let env = sanitized_env(&[
            OsString::from("OPENAI_API_KEY=sk-1234567890"),
            OsString::from("PATH=/usr/bin"),
        ]);

        assert_eq!(env.get("PATH").map(String::as_str), Some("/usr/bin"));
        let api_key = env.get("OPENAI_API_KEY").unwrap();
        assert_eq!(api_key, "****");
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
