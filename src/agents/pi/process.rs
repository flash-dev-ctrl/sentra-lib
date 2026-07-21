use crate::agents::process::{ProcessInfo, cmdline_has_path_components, matches_binary_names};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_process)
}

pub(super) fn matches_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["pi", "pi.exe"])
        || (matches_binary_names(process, &["node", "node.exe"])
            && cmdline_has_path_components(process, &["@earendil-works", "pi-coding-agent"]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_exact_npm_wrapper_path() {
        assert_node_process(
            "/usr/lib/node_modules/@earendil-works/pi-coding-agent/dist/cli.js",
            true,
        );
        assert_node_process(
            "/usr/lib/node_modules/@earendil-works/pi-coding-agent-helper/dist/cli.js",
            false,
        );
    }

    fn assert_node_process(script: &str, expected: bool) {
        let cmdline = vec!["node".to_string(), script.to_string()];
        let process = ProcessInfo {
            name: "node",
            cmdline: &cmdline,
            path: None,
        };
        assert_eq!(matches_process(&process), expected);
    }
}
