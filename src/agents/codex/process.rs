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

pub(super) fn matcher(agent_name: &str) -> crate::agents::process::ProcessMatcher {
    if agent_name == crate::agents::entries::CODEX_APP_AGENT_ENTRY.name {
        matches_app_process
    } else {
        matches_cli_process
    }
}

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_cli_process)
}

pub(crate) fn app_process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_app_process)
}

fn matches_cli_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, CODEX_BINARY_NAMES)
        && !process.path.is_some_and(is_codex_desktop_path)
}

fn matches_app_process(process: &ProcessInfo<'_>) -> bool {
    process.path.is_some_and(is_codex_desktop_path)
}

fn is_codex_desktop_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    !path_has_component(path, &["bin"])
        && is_binary_name(file_name, CODEX_DESKTOP_BINARY_NAMES)
        && path_has_component(path, CODEX_DESKTOP_PATH_COMPONENTS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_codex_process_by_binary_name() {
        assert_process_match("codex", true, "codex", &[], None);
        assert_process_match("codex", true, "codex.exe", &[], None);
        assert_process_match("codex", true, "Codex.exe", &[], None);
    }

    #[test]
    fn matches_codex_process_by_first_command() {
        assert_process_match(
            "codex",
            true,
            "node",
            &[r"C:\Users\me\AppData\Local\Programs\OpenAI\Codex\codex.exe"],
            None,
        );
        assert_process_match("codex", true, "node", &["/usr/local/bin/codex"], None);
    }

    #[test]
    fn does_not_match_codex_as_substring() {
        assert_process_match("codex", false, "my-codex-helper", &[], None);
        assert_process_match(
            "codex",
            false,
            "node",
            &["/usr/local/bin/my-codex-helper"],
            None,
        );
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

        assert_process_match("codex-app", true, "ChatGPT.exe", &[], Some(&chatgpt_path));
        assert_process_match("codex", false, "ChatGPT.exe", &[], Some(&chatgpt_path));
        assert_process_match("codex-app", false, "ChatGPT.exe", &[], Some(&temp_path));
    }

    #[test]
    fn separates_codex_cli_bin_from_desktop_app() {
        let cli_path = Path::new("Users")
            .join("me")
            .join("AppData")
            .join("Local")
            .join("Programs")
            .join("OpenAI")
            .join("Codex")
            .join("bin")
            .join("codex.exe");
        let app_path = Path::new("Applications")
            .join("Codex.app")
            .join("Contents")
            .join("MacOS")
            .join("Codex");

        assert_process_match("codex", true, "codex.exe", &[], Some(&cli_path));
        assert_process_match("codex-app", false, "codex.exe", &[], Some(&cli_path));
        assert_process_match("codex", false, "Codex", &[], Some(&app_path));
        assert_process_match("codex-app", true, "Codex", &[], Some(&app_path));
    }

    fn assert_process_match(
        agent_name: &str,
        expected: bool,
        name: &str,
        cmdline: &[&str],
        path: Option<&Path>,
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
        assert_eq!((matcher(agent_name))(&process), expected);
    }
}
