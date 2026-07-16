use std::path::Path;

use crate::agents::process::{
    ProcessInfo, is_binary_name, matches_binary_names, path_has_component,
};
use crate::interfaces::ProcessData;

const CODEX_BINARY_NAMES: &[&str] = &["codex", "codex.exe"];
const CODEX_DESKTOP_BINARY_NAMES: &[&str] = &["codex", "codex.exe", "chatgpt", "chatgpt.exe"];
const CODEX_DESKTOP_PATH_COMPONENTS: &[&str] = &[
    "codex",
    "chatgpt",
    "codex.app",
    "chatgpt.app",
    "openai",
    "openai codex",
    "openai chatgpt",
];

pub(super) fn matches_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, CODEX_BINARY_NAMES)
        || process.path.is_some_and(is_codex_desktop_path)
}

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_process)
}

fn is_codex_desktop_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    is_binary_name(file_name, CODEX_DESKTOP_BINARY_NAMES)
        && path_has_component(path, CODEX_DESKTOP_PATH_COMPONENTS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_codex_process_by_binary_name() {
        assert_matches_process("codex", &[], None);
        assert_matches_process("codex.exe", &[], None);
        assert_matches_process("Codex.exe", &[], None);
    }

    #[test]
    fn matches_codex_process_by_first_command() {
        assert_matches_process(
            "node",
            &[r"C:\Users\me\AppData\Local\Programs\OpenAI\Codex\codex.exe"],
            None,
        );
        assert_matches_process("node", &["/usr/local/bin/codex"], None);
    }

    #[test]
    fn does_not_match_codex_as_substring() {
        assert_not_matches_process("my-codex-helper", &[], None);
        assert_not_matches_process("node", &["/usr/local/bin/my-codex-helper"], None);
    }

    #[test]
    fn matches_chatgpt_only_from_desktop_app_path() {
        let chatgpt_path = Path::new("Users")
            .join("me")
            .join("AppData")
            .join("Local")
            .join("Programs")
            .join("OpenAI")
            .join("ChatGPT")
            .join("ChatGPT.exe");
        let temp_path = Path::new("temp").join("ChatGPT.exe");

        assert_matches_process("ChatGPT.exe", &[], Some(&chatgpt_path));
        assert_not_matches_process("ChatGPT.exe", &[], Some(&temp_path));
    }

    fn assert_matches_process(name: &str, cmdline: &[&str], path: Option<&Path>) {
        let cmdline = cmdline
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let process = ProcessInfo {
            name,
            cmdline: &cmdline,
            path,
        };
        assert!(matches_process(&process));
    }

    fn assert_not_matches_process(name: &str, cmdline: &[&str], path: Option<&Path>) {
        let cmdline = cmdline
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let process = ProcessInfo {
            name,
            cmdline: &cmdline,
            path,
        };
        assert!(!matches_process(&process));
    }
}
