use crate::agents::process::{ProcessInfo, matches_binary_names, path_has_component};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_process)
}

pub(super) fn matches_process(process: &ProcessInfo<'_>) -> bool {
    if !matches_binary_names(process, &["claude", "claude.exe"]) {
        return false;
    }
    process.path.is_none_or(|path| {
        let is_desktop_app = path_has_component(path, &["claude.app", "claude-3p.app"])
            || (path_has_component(path, &["appdata"])
                && path_has_component(path, &["claude", "claude-3p"]));
        !is_desktop_app
    })
}
