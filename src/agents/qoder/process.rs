use crate::agents::process::{ProcessInfo, matches_binary_names};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_any_process)
}

pub(super) fn matcher(agent_name: &str) -> crate::agents::process::ProcessMatcher {
    if agent_name == "qoder-cn" {
        matches_cn_process
    } else {
        matches_qoder_process
    }
}

fn matches_any_process(process: &ProcessInfo<'_>) -> bool {
    matches_qoder_process(process) || matches_cn_process(process)
}

fn matches_qoder_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["qodercli", "qodercli.exe"])
}

fn matches_cn_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["qoderclicn", "qoderclicn.exe"])
}

#[cfg(test)]
mod tests {
    use crate::agents::process::ProcessInfo;

    #[test]
    fn matches_qoder_cn_separately() {
        let cmdline = vec!["qoderclicn".to_string()];
        let process = ProcessInfo {
            name: "qoderclicn",
            cmdline: &cmdline,
            path: None,
        };
        assert!((super::matcher("qoder-cn"))(&process));
        assert!(!(super::matcher("qoder"))(&process));
    }
}
