use crate::agents::process::{matches_binary_names, path_has_component, ProcessInfo};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_process)
}

pub(crate) fn matches_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(
        process,
        &["agy", "agy.exe", "Antigravity", "Antigravity.exe"],
    ) || process
        .path
        .is_some_and(|path| path_has_component(path, &["antigravity"]))
}
