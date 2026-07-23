use crate::agents::process::{ProcessInfo, matches_binary_names};
use crate::agents::qoder::surface;
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_any_process)
}

pub(super) fn matcher(agent_name: &str) -> crate::agents::process::ProcessMatcher {
    if surface::is_cn(agent_name) {
        matches_cn_process
    } else {
        matches_en_process
    }
}

fn matches_any_process(process: &ProcessInfo<'_>) -> bool {
    matches_en_process(process) || matches_cn_process(process)
}

fn matches_en_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(
        process,
        &["QoderWork", "QoderWork.exe", "qoderwork", "qoderwork.exe"],
    )
}

fn matches_cn_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(
        process,
        &[
            "QoderWorkCN",
            "QoderWorkCN.exe",
            "Qoder Work CN",
            "Qoder Work CN.exe",
            "qoderworkcn",
            "qoderworkcn.exe",
        ],
    )
}
