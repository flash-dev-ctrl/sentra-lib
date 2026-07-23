use crate::agents::codebuddy::surface;
use crate::agents::process::{
    ProcessInfo, cmdline_has_path_components, matches_binary_names, path_has_component,
    process_has_ide_extension,
};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_cli_process)
}

pub(crate) fn ide_process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_any_ide_process)
}

pub(crate) fn ide_extension_process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_ide_extension_process)
}

pub(super) fn matcher(agent_name: &str) -> crate::agents::process::ProcessMatcher {
    if surface::is_ide_extension(agent_name) {
        matches_ide_extension_process
    } else if surface::is_ide(agent_name) {
        if surface::is_cn(agent_name) {
            matches_cn_ide_process
        } else {
            matches_en_ide_process
        }
    } else {
        matches_cli_process
    }
}

fn matches_cli_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(
        process,
        &[
            "codebuddy",
            "codebuddy.exe",
            "codebuddy.ps1",
            "codebuddy-cli",
            "codebuddy-cli.exe",
            "codebuddy-cli.ps1",
        ],
    ) || (matches_binary_names(process, &["node", "node.exe"])
        && cmdline_has_path_components(process, &["@tencent-ai", "codebuddy-code"]))
}

fn matches_any_ide_process(process: &ProcessInfo<'_>) -> bool {
    matches_en_ide_process(process) || matches_cn_ide_process(process)
}

fn matches_en_ide_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["CodeBuddy", "CodeBuddy.exe"])
        || process
            .path
            .is_some_and(|path| path_has_component(path, &["CodeBuddy"]))
}

fn matches_cn_ide_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(
        process,
        &["CodeBuddy CN", "CodeBuddy CN.exe", "buddycn", "buddycn.exe"],
    ) || process
        .path
        .is_some_and(|path| path_has_component(path, &["CodeBuddy CN"]))
}

fn matches_ide_extension_process(process: &ProcessInfo<'_>) -> bool {
    process_has_ide_extension(process, surface::CODEBUDDY_IDE_EXTENSION_ID)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn matches_exact_npm_wrapper_path() {
        assert_node_process(
            r"C:\npm\node_modules\@tencent-ai\codebuddy-code\dist\cli.js",
            true,
        );
        assert_node_process(
            r"C:\npm\node_modules\@tencent-ai\codebuddy-code-beta\dist\cli.js",
            false,
        );
    }

    fn assert_node_process(script: &str, expected: bool) {
        let cmdline = vec!["node.exe".to_string(), script.to_string()];
        let process = ProcessInfo {
            name: "node.exe",
            cmdline: &cmdline,
            path: None,
        };
        assert_eq!(matches_cli_process(&process), expected);
    }

    #[test]
    fn separates_cn_ide_process_from_cli() {
        let app = Path::new("Users")
            .join("me")
            .join("AppData")
            .join("Local")
            .join("Programs")
            .join("CodeBuddy CN")
            .join("CodeBuddy CN.exe");

        let process = ProcessInfo {
            name: "CodeBuddy CN.exe",
            cmdline: &[],
            path: Some(app.as_path()),
        };

        assert!(matches_cn_ide_process(&process));
        assert!(!matches_cli_process(&process));
    }

    #[test]
    fn matches_ide_extension_process_by_extension_path() {
        let cmdline = vec![
            "node.exe".to_string(),
            r"C:\Users\me\.vscode\extensions\tencent-cloud.coding-copilot-1.0.0\dist\extension.js"
                .to_string(),
        ];
        let process = ProcessInfo {
            name: "node.exe",
            cmdline: &cmdline,
            path: None,
        };

        assert!(matches_ide_extension_process(&process));
    }
}
