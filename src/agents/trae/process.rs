use crate::agents::process::{
    ProcessInfo, matches_binary_names, path_has_component, process_has_ide_extension,
};
use crate::agents::trae::surface;
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_any_process)
}

pub(crate) fn work_process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_any_work_process)
}

pub(crate) fn vscode_plugin_process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_vscode_plugin_process)
}

pub(super) fn matcher(agent_name: &str) -> crate::agents::process::ProcessMatcher {
    match surface::surface(agent_name) {
        surface::TraeSurface::Ide(surface::TraeEdition::Cn) => matches_cn_ide_process,
        surface::TraeSurface::Work(surface::TraeEdition::En) => matches_en_work_process,
        surface::TraeSurface::Work(surface::TraeEdition::Cn) => matches_cn_work_process,
        surface::TraeSurface::VscodePlugin => matches_vscode_plugin_process,
        _ => matches_en_ide_process,
    }
}

fn matches_any_process(process: &ProcessInfo<'_>) -> bool {
    matches_en_ide_process(process)
        || matches_cn_ide_process(process)
        || matches_any_work_process(process)
        || matches_vscode_plugin_process(process)
}

fn matches_en_ide_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["trae-cli", "trae-cli.exe", "Trae", "Trae.exe"])
        || process
            .path
            .is_some_and(|path| path_has_component(path, &["trae", "trae.app"]))
}

fn matches_cn_ide_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(process, &["Trae CN", "Trae CN.exe", "TraeCN", "TraeCN.exe"])
        || process
            .path
            .is_some_and(|path| path_has_component(path, &["trae cn", "traecn", "trae cn.app"]))
}

fn matches_any_work_process(process: &ProcessInfo<'_>) -> bool {
    matches_en_work_process(process) || matches_cn_work_process(process)
}

fn matches_en_work_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(
        process,
        &[
            "TRAE SOLO",
            "TRAE SOLO.exe",
            "Trae Work",
            "Trae Work.exe",
            "TraeWork",
            "TraeWork.exe",
            "traework",
            "traework.exe",
        ],
    ) || process.path.is_some_and(|path| {
        path_has_component(
            path,
            &[
                "trae work",
                "traework",
                "trae solo",
                "trae work.app",
                "trae solo.app",
            ],
        )
    })
}

fn matches_cn_work_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(
        process,
        &[
            "TRAE SOLO CN",
            "TRAE SOLO CN.exe",
            "Trae CN Work",
            "Trae CN Work.exe",
            "Trae Work CN",
            "Trae Work CN.exe",
            "TraeWorkCN",
            "TraeWorkCN.exe",
            "traeworkcn",
            "traeworkcn.exe",
        ],
    ) || process.path.is_some_and(|path| {
        path_has_component(
            path,
            &[
                "trae solo cn",
                "trae solo_cn",
                "trae cn work",
                "trae work cn",
                "traeworkcn",
                "trae cn work.app",
                "trae work cn.app",
                "trae solo cn.app",
            ],
        )
    })
}

fn matches_vscode_plugin_process(process: &ProcessInfo<'_>) -> bool {
    surface::TRAE_VSCODE_EXTENSION_IDS
        .iter()
        .any(|extension_id| process_has_ide_extension(process, extension_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_substring_helpers() {
        let cmdline = vec!["my-trae-cli-helper".to_string()];
        let process = ProcessInfo {
            name: "node",
            cmdline: &cmdline,
            path: None,
        };
        assert!(!matches_any_process(&process));
    }

    #[test]
    fn separates_ide_work_and_plugin_processes() {
        assert_process_match("trae-ide", true, "Trae.exe", &[], None);
        assert_process_match("trae-work", false, "Trae.exe", &[], None);
        assert_process_match("trae-work", true, "TRAE SOLO.exe", &[], None);
        assert_process_match("trae-cn-work", false, "TRAE SOLO.exe", &[], None);
        assert_process_match("trae-cn-work", true, "TRAE SOLO CN.exe", &[], None);

        let cmdline = [
            "node.exe",
            r"C:\Users\me\.vscode\extensions\marscode.marscode-extension-1.7.3\dist\extension.js",
        ];
        assert_process_match("trae-vscode-plugin", true, "node.exe", &cmdline, None);
        assert_process_match("trae-ide", false, "node.exe", &cmdline, None);
    }

    fn assert_process_match(
        agent_name: &str,
        expected: bool,
        name: &str,
        cmdline: &[&str],
        path: Option<&std::path::Path>,
    ) {
        let cmdline = cmdline
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let process = ProcessInfo {
            name,
            cmdline: &cmdline,
            path,
        };
        assert_eq!((matcher(agent_name))(&process), expected);
    }
}
