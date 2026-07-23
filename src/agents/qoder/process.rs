use crate::agents::process::{ProcessInfo, matches_binary_names};
use crate::agents::qoder::surface;
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_any_process)
}

pub(super) fn matcher(agent_name: &str) -> crate::agents::process::ProcessMatcher {
    match surface::surface(agent_name) {
        surface::QoderSurface::Cli(surface::QoderEdition::Cn) => matches_cn_cli_process,
        surface::QoderSurface::Ide(surface::QoderEdition::En) => matches_en_ide_process,
        surface::QoderSurface::Ide(surface::QoderEdition::Cn) => matches_cn_ide_process,
        _ => matches_en_cli_process,
    }
}

fn matches_any_process(process: &ProcessInfo<'_>) -> bool {
    matches_en_cli_process(process)
        || matches_cn_cli_process(process)
        || matches_en_ide_process(process)
        || matches_cn_ide_process(process)
}

fn matches_en_cli_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["qodercli", "qodercli.exe"])
}

fn matches_cn_cli_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["qoderclicn", "qoderclicn.exe"])
}

fn matches_en_ide_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["Qoder", "Qoder.exe"])
}

fn matches_cn_ide_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(
        process,
        &["QoderCN", "QoderCN.exe", "Qoder CN", "Qoder CN.exe"],
    )
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
        assert!((super::matcher("qoder-cn-cli"))(&process));
        assert!(!(super::matcher("qoder-cli"))(&process));
    }

    #[test]
    fn matches_qoder_ide_separately_from_cli() {
        let cmdline = vec!["Qoder.exe".to_string()];
        let process = ProcessInfo {
            name: "Qoder",
            cmdline: &cmdline,
            path: None,
        };
        assert!((super::matcher("qoder-ide"))(&process));
        assert!(!(super::matcher("qoder-cli"))(&process));
    }
}
