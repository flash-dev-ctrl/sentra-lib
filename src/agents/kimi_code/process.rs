use crate::agents::process::{
    ProcessInfo, cmdline_has_path_components, matches_binary_names, path_has_component,
    process_has_ide_extension,
};
use crate::interfaces::ProcessData;

const KIMI_BINARY_NAMES: &[&str] = &["kimi", "kimi.exe"];
const NODE_BINARY_NAMES: &[&str] = &["node", "node.exe"];
const WEBBRIDGE_BINARY_NAMES: &[&str] = &["kimi-webbridge", "kimi-webbridge.exe"];

pub(super) fn matcher(agent_name: &str) -> crate::agents::process::ProcessMatcher {
    if agent_name == crate::agents::entries::KIMI_APP_AGENT_ENTRY.name {
        matches_app_process
    } else if agent_name == crate::agents::entries::KIMI_CLI_IDE_AGENT_ENTRY.name {
        matches_ide_process
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
    let is_cli = matches_binary_names(process, KIMI_BINARY_NAMES)
        || (matches_binary_names(process, NODE_BINARY_NAMES)
            && cmdline_has_path_components(process, &["@moonshot-ai", "kimi-code"]));
    is_cli
        && !matches_app_process(process)
        && !process_has_ide_extension(
            process,
            crate::agents::kimi_code::KIMI_CODE_IDE_EXTENSION_ID,
        )
}

fn matches_app_process(process: &ProcessInfo<'_>) -> bool {
    process
        .path
        .is_some_and(|path| path_has_component(path, &["kimi-desktop"]))
        || ((matches_binary_names(process, KIMI_BINARY_NAMES)
            || matches_binary_names(process, NODE_BINARY_NAMES))
            && cmdline_has_path_components(process, &["kimi-desktop"]))
        || matches_binary_names(process, WEBBRIDGE_BINARY_NAMES)
}

fn matches_ide_process(process: &ProcessInfo<'_>) -> bool {
    process_has_ide_extension(
        process,
        crate::agents::kimi_code::KIMI_CODE_IDE_EXTENSION_ID,
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn matches_cli_binary_and_npm_entrypoint() {
        assert_process_match("kimi-cli", true, "kimi", &[], None);
        assert_process_match("kimi-cli", true, "Kimi.exe", &[], None);
        assert_process_match(
            "kimi-cli",
            true,
            "node.exe",
            &[
                "node.exe",
                r"C:\Users\me\AppData\Roaming\npm\node_modules\@moonshot-ai\kimi-code\dist\main.mjs",
            ],
            Some(Path::new(r"C:\Program Files\nodejs\node.exe")),
        );
    }

    #[test]
    fn separates_desktop_process_tree_from_cli() {
        let app = Path::new(r"C:\Users\me\AppData\Local\Programs\kimi-desktop\Kimi.exe");
        let runtime = Path::new(
            r"C:\Users\me\AppData\Local\Programs\kimi-desktop\resources\runtime\node.exe",
        );

        assert_process_match("kimi-app", true, "Kimi.exe", &[], Some(app));
        assert_process_match("kimi-cli", false, "Kimi.exe", &[], Some(app));
        assert_process_match("kimi-app", true, "node.exe", &[], Some(runtime));
        assert_process_match("kimi-cli", false, "node.exe", &[], Some(runtime));
        assert_process_match(
            "kimi-app",
            true,
            "Kimi.exe",
            &[r"C:\Users\me\AppData\Local\Programs\kimi-desktop\Kimi.exe"],
            None,
        );
        assert_process_match(
            "kimi-cli",
            false,
            "Kimi.exe",
            &[r"C:\Users\me\AppData\Local\Programs\kimi-desktop\Kimi.exe"],
            None,
        );
    }

    #[test]
    fn attributes_webbridge_to_desktop_app() {
        let path = Path::new(r"C:\Users\me\.kimi-webbridge\bin\kimi-webbridge.exe");

        assert_process_match("kimi-app", true, "kimi-webbridge.exe", &[], Some(path));
        assert_process_match("kimi-cli", false, "kimi-webbridge.exe", &[], Some(path));
    }

    #[test]
    fn only_attributes_explicit_extension_processes_to_ide() {
        let extension_script = r"C:\Users\me\.vscode\extensions\moonshot-ai.kimi-code-0.6.4-win32-x64\dist\extension.js";
        let cmdline = ["node.exe", extension_script];

        assert_process_match("kimi-cli-ide", true, "node.exe", &cmdline, None);
        assert_process_match("kimi-cli", false, "node.exe", &cmdline, None);
        assert_process_match("kimi-cli-ide", false, "Code.exe", &[], None);
    }

    #[test]
    fn does_not_match_similar_names() {
        assert_process_match("kimi-cli", false, "kimi-code-helper", &[], None);
        assert_process_match(
            "kimi-cli",
            false,
            "node",
            &["node", "/opt/@moonshot-ai/kimi-code-helper/main.mjs"],
            None,
        );
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
