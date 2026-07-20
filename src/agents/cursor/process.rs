use crate::agents::process::{ProcessInfo, matches_binary_names, path_has_component};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_process)
}

pub(super) fn matches_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["cursor", "cursor.exe"])
        || matches_binary_names(process, &["agent", "agent.exe"])
            && process
                .path
                .is_some_and(|path| path_has_component(path, &["cursor"]))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn matches_cursor_process_names() {
        assert!(matches_process(&process("Cursor.exe")));
        assert!(matches_process(&ProcessInfo {
            name: "agent",
            cmdline: &[],
            path: Some(Path::new("/Applications/Cursor/agent")),
        }));
        assert!(!matches_process(&process("agent")));
        assert!(!matches_process(&process("cursor-helper")));
    }

    fn process(name: &str) -> ProcessInfo<'_> {
        ProcessInfo {
            name,
            cmdline: &[],
            path: None,
        }
    }
}
