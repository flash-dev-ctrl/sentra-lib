use crate::agents::process::{ProcessInfo, matches_binary_names};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_process)
}

pub(crate) fn matches_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(
        process,
        &["coder", "coder.exe", "code-server", "code-server.exe"],
    )
}

#[cfg(test)]
mod tests {
    use crate::agents::process::ProcessInfo;

    use super::matches_process;

    #[test]
    fn matches_coder_and_code_server_only() {
        assert!(matches_process(&info("coder")));
        assert!(matches_process(&info("code-server")));
        assert!(!matches_process(&info("code")));
    }

    fn info(name: &str) -> ProcessInfo<'_> {
        ProcessInfo {
            name,
            cmdline: &[],
            path: None,
        }
    }
}
