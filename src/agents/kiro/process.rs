use crate::agents::process::{matches_binary_names, ProcessInfo};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_process)
}

pub(super) fn matches_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["kiro-cli", "kiro-cli.exe", "kiro", "kiro.exe"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_kiro_process_names() {
        assert!(matches_process(&process("kiro-cli")));
        assert!(matches_process(&process("Kiro.exe")));
        assert!(!matches_process(&process("kiro-helper")));
    }

    fn process(name: &str) -> ProcessInfo<'_> {
        ProcessInfo {
            name,
            cmdline: &[],
            path: None,
        }
    }
}
