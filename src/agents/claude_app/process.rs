use crate::agents::process::{ProcessInfo, matches_binary_names, path_has_component};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_process)
}

pub(super) fn matches_process(process: &ProcessInfo<'_>) -> bool {
    let Some(path) = process.path else {
        return false;
    };
    matches_binary_names(process, &["claude", "claude.exe"])
        && (path_has_component(path, &["claude.app", "claude-3p.app"])
            || (path_has_component(path, &["appdata"])
                && path_has_component(path, &["claude", "claude-3p"]))
            || (path_has_component(path, &["windowsapps"])
                && path
                    .parent()
                    .and_then(std::path::Path::file_name)
                    .and_then(|value| value.to_str())
                    .is_some_and(|parent| parent.eq_ignore_ascii_case("app"))))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn matches_store_main_process() {
        let path = Path::new("Program Files")
            .join("WindowsApps")
            .join("Claude_1.0.0.0_x64__pzs8sxrjxfjjc")
            .join("app")
            .join("Claude.exe");
        let cmdline = Vec::new();
        let process = ProcessInfo {
            name: "Claude",
            cmdline: &cmdline,
            path: Some(&path),
        };

        assert!(matches_process(&process));
    }
}
