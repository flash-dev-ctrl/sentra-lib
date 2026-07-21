use crate::agents::process::{
    ProcessInfo, is_binary_name, matches_binary_names, path_has_component,
};
use crate::interfaces::ProcessData;

pub(crate) fn process_data() -> Vec<ProcessData> {
    crate::agents::process::process_data(matches_process)
}

pub(crate) fn matches_process(process: &ProcessInfo<'_>) -> bool {
    matches_binary_names(
        process,
        &[
            "code",
            "code.exe",
            "Code.exe",
            "code-insiders",
            "Code - Insiders.exe",
        ],
    ) || process.path.is_some_and(|path| {
        path.file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| is_binary_name(name, &["Electron"]))
            && path_has_component(
                path,
                &[
                    "Visual Studio Code.app",
                    "Visual Studio Code - Insiders.app",
                ],
            )
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn matches_macos_electron_bundle_process() {
        let path = Path::new("Applications")
            .join("Visual Studio Code.app")
            .join("Contents")
            .join("MacOS")
            .join("Electron");
        let cmdline = Vec::new();
        let process = ProcessInfo {
            name: "Electron",
            cmdline: &cmdline,
            path: Some(&path),
        };

        assert!(matches_process(&process));
    }
}
