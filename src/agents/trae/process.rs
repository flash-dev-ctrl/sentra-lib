use crate::agents::process::{ProcessInfo, matches_binary_names, path_has_component};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_process)
}

pub(super) fn matches_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["trae-cli", "trae-cli.exe", "Trae", "Trae.exe"])
        || process
            .path
            .is_some_and(|path| path_has_component(path, &["trae", "trae.app"]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_substring_helpers() {
        let cmdline = vec!["my-trae-cli-helper".to_string()];
        let process = ProcessInfo {
            name: "node",
            cmdline: &cmdline,
            path: None,
        };
        assert!(!matches_process(&process));
    }
}
