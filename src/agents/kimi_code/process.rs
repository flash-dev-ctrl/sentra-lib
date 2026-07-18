use crate::agents::process::{ProcessInfo, matches_binary_names};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_process)
}

pub(super) fn matches_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["kimi", "kimi.exe"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_kimi_process_by_binary_name() {
        assert_matches_process("kimi", &[]);
        assert_matches_process("kimi.exe", &[]);
        assert_matches_process("Kimi.exe", &[]);
        assert_matches_process("node", &["/usr/local/bin/kimi"]);
    }

    #[test]
    fn does_not_match_kimi_as_substring() {
        assert_not_matches_process("kimi-code-helper", &[]);
        assert_not_matches_process("node", &["/usr/local/bin/kimi-helper"]);
    }

    fn assert_matches_process(name: &str, cmdline: &[&str]) {
        let cmdline = cmdline
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let process = ProcessInfo {
            name,
            cmdline: &cmdline,
            path: None,
        };
        assert!(matches_process(&process));
    }

    fn assert_not_matches_process(name: &str, cmdline: &[&str]) {
        let cmdline = cmdline
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let process = ProcessInfo {
            name,
            cmdline: &cmdline,
            path: None,
        };
        assert!(!matches_process(&process));
    }
}
