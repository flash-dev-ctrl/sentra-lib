use crate::agents::process::{
    ProcessInfo, cmdline_has_path_components, matches_binary_names, path_has_component,
    process_has_ide_extension,
};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_cli_process)
}

pub(crate) fn ide_process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_ide_process)
}

pub(super) fn matcher(agent_name: &str) -> crate::agents::process::ProcessMatcher {
    if agent_name == crate::agents::entries::CLAUDE_CODE_IDE_AGENT_ENTRY.name {
        matches_ide_process
    } else {
        matches_cli_process
    }
}

fn matches_cli_process(process: &ProcessInfo<'_>) -> bool {
    let is_cli = matches_binary_names(process, &["claude", "claude.exe"])
        || (matches_binary_names(process, &["node", "node.exe"])
            && cmdline_has_path_components(process, &["@anthropic-ai", "claude-code"]));
    is_cli
        && !process.path.is_some_and(is_claude_desktop_path)
        && !process_has_ide_extension(
            process,
            crate::agents::claude_cli::CLAUDE_CODE_IDE_EXTENSION_ID,
        )
}

fn matches_ide_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["claude", "claude.exe"])
        && process_has_ide_extension(
            process,
            crate::agents::claude_cli::CLAUDE_CODE_IDE_EXTENSION_ID,
        )
}

fn is_claude_desktop_path(path: &std::path::Path) -> bool {
    path_has_component(path, &["claude.app", "claude-3p.app"])
        || (path_has_component(path, &["appdata"])
            && path_has_component(path, &["claude", "claude-3p"]))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn separates_claude_ide_extension_bundle_from_cli() {
        let ide_path = Path::new("Users")
            .join("me")
            .join(".vscode")
            .join("extensions")
            .join("anthropic.claude-code-2.1.0-win32-x64")
            .join("resources")
            .join("native-binary")
            .join("claude.exe");
        let cli_path = Path::new("usr").join("local").join("bin").join("claude");

        assert_process_match("claude-code-ide", true, Some(&ide_path));
        assert_process_match("claude-cli", false, Some(&ide_path));
        assert_process_match("claude-cli", true, Some(&cli_path));
        assert_process_match("claude-code-ide", false, Some(&cli_path));
    }

    #[test]
    fn matches_exact_npm_wrapper_path() {
        assert_node_process(
            "/usr/lib/node_modules/@anthropic-ai/claude-code/cli.js",
            true,
        );
        assert_node_process(
            "/usr/lib/node_modules/@anthropic-ai/claude-code-helper/cli.js",
            false,
        );
    }

    fn assert_process_match(agent_name: &str, expected: bool, path: Option<&Path>) {
        let process = ProcessInfo {
            name: "claude.exe",
            cmdline: &[],
            path,
        };
        assert_eq!((matcher(agent_name))(&process), expected);
    }

    fn assert_node_process(script: &str, expected: bool) {
        let cmdline = vec!["node".to_string(), script.to_string()];
        let process = ProcessInfo {
            name: "node",
            cmdline: &cmdline,
            path: None,
        };
        assert_eq!(matches_cli_process(&process), expected);
    }
}
