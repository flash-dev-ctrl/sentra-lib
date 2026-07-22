use std::path::Path;

use crate::agents::process::{
    ProcessInfo, is_binary_name, matches_binary_names, path_has_component,
    process_has_ide_extension,
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
    if agent_name == crate::agents::entries::CODEX_CLI_IDE_AGENT_ENTRY.name {
        matches_ide_process
    } else if agent_name == crate::agents::entries::CODEX_APP_AGENT_ENTRY.name {
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

pub(crate) fn ide_process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_ide_process)
}

fn matches_cli_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, CODEX_BINARY_NAMES)
        && !process.path.is_some_and(is_codex_desktop_main)
        && !process_has_ide_extension(process, crate::agents::codex::CODEX_IDE_EXTENSION_ID)
}

fn matches_app_process(process: &ProcessInfo<'_>) -> bool {
    process.path.is_some_and(is_codex_desktop_main)
}

fn matches_ide_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, CODEX_BINARY_NAMES)
        && process_has_ide_extension(process, crate::agents::codex::CODEX_IDE_EXTENSION_ID)
}

fn is_codex_desktop_main(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    if !is_binary_name(file_name, CODEX_DESKTOP_BINARY_NAMES) {
        return false;
    }
    if path_has_component(path, &["codex.app", "chatgpt.app"]) {
        return true;
    }
    let parent = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str());
    (parent.is_some_and(|parent| is_binary_name(parent, &["app"]))
        && is_windows_store_package_path(path))
        || (!path_has_component(path, &["bin", "resources"])
            && path_has_component(path, CODEX_DESKTOP_PATH_COMPONENTS))
}

fn is_windows_store_package_path(path: &Path) -> bool {
    path_has_component(path, &["windowsapps"])
        && path.components().any(|component| {
            let component = component.as_os_str().to_string_lossy().to_ascii_lowercase();
            component.starts_with("openai.codex_") || component.starts_with("openai.chatgpt_")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_codex_process_by_binary_name() {
        assert_process_match("codex-cli", true, "codex", &[], None);
        assert_process_match("codex-cli", true, "codex.exe", &[], None);
        assert_process_match("codex-cli", true, "Codex.exe", &[], None);
    }

    #[test]
    fn matches_codex_process_by_first_command() {
        assert_process_match(
            "codex-cli",
            true,
            "node",
            &[r"C:\Users\me\AppData\Local\Programs\OpenAI\Codex\codex.exe"],
            None,
        );
        assert_process_match("codex-cli", true, "node", &["/usr/local/bin/codex"], None);
    }

    #[test]
    fn does_not_match_codex_as_substring() {
        assert_process_match("codex-cli", false, "my-codex-helper", &[], None);
        assert_process_match(
            "codex-cli",
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
        assert_process_match("codex-cli", false, "ChatGPT.exe", &[], Some(&chatgpt_path));
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

        assert_process_match("codex-cli", true, "codex.exe", &[], Some(&cli_path));
        assert_process_match("codex-app", false, "codex.exe", &[], Some(&cli_path));
        assert_process_match("codex-cli", false, "Codex", &[], Some(&app_path));
        assert_process_match("codex-app", true, "Codex", &[], Some(&app_path));
    }

    #[test]
    fn separates_codex_ide_extension_bundle_from_cli_and_desktop_app() {
        let ide_path = Path::new("Users")
            .join("me")
            .join(".devin")
            .join("extensions")
            .join("openai.chatgpt-1.2.3-win32-x64")
            .join("bin")
            .join("windows-x86_64")
            .join("codex.exe");

        assert_process_match("codex-cli-ide", true, "codex.exe", &[], Some(&ide_path));
        assert_process_match("codex-cli", false, "codex.exe", &[], Some(&ide_path));
        assert_process_match("codex-app", false, "codex.exe", &[], Some(&ide_path));
    }

    #[test]
    fn separates_store_main_process_from_bundled_sidecar() {
        let app = Path::new("Program Files")
            .join("WindowsApps")
            .join("OpenAI.Codex_1.0.0.0_x64__2p2nqsd0c76g0")
            .join("app");
        let main = app.join("Codex.exe");
        let sidecar = app.join("resources").join("codex.exe");

        assert_process_match("codex-app", true, "Codex", &[], Some(&main));
        assert_process_match("codex-cli", false, "Codex", &[], Some(&main));
        assert_process_match("codex-app", false, "codex", &[], Some(&sidecar));
        assert_process_match("codex-cli", true, "codex", &[], Some(&sidecar));
    }

    #[test]
    fn keeps_regular_app_directory_codex_as_cli() {
        let path = Path::new("tools").join("app").join("codex.exe");

        assert_process_match("codex-app", false, "codex", &[], Some(&path));
        assert_process_match("codex-cli", true, "codex", &[], Some(&path));
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
