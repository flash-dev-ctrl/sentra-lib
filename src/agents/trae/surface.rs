use std::path::{Path, PathBuf};

use crate::agents::install_status::{env_path, hidden_home_parent};

pub(super) const TRAE_VSCODE_EXTENSION_IDS: &[&str] = &["marscode.marscode-extension"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TraeEdition {
    En,
    Cn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TraeSurface {
    Ide(TraeEdition),
    Work(TraeEdition),
    VscodePlugin,
}

pub(super) fn surface(agent_name: &str) -> TraeSurface {
    match agent_name {
        "trae-cn" | "trae-cn-ide" => TraeSurface::Ide(TraeEdition::Cn),
        "trae-work" | "traework" | "trae-solo" => TraeSurface::Work(TraeEdition::En),
        "trae-cn-work" | "trae-work-cn" | "traeworkcn" | "trae-solo-cn" => {
            TraeSurface::Work(TraeEdition::Cn)
        }
        "trae-vscode-plugin" | "trae-plugin" | "trae-ide-plugin" => TraeSurface::VscodePlugin,
        _ => TraeSurface::Ide(TraeEdition::En),
    }
}

pub(super) fn title(agent_name: &str) -> &'static str {
    match surface(agent_name) {
        TraeSurface::Ide(TraeEdition::En) => "Trae IDE",
        TraeSurface::Ide(TraeEdition::Cn) => "Trae CN IDE",
        TraeSurface::Work(TraeEdition::En) => "Trae Work",
        TraeSurface::Work(TraeEdition::Cn) => "Trae CN Work",
        TraeSurface::VscodePlugin => "TRAE VS Code Plugin",
    }
}

pub(super) fn author(agent_name: &str) -> &'static str {
    if is_cn(agent_name) {
        "Beijing Yinli Catapult Technology Co., Ltd."
    } else {
        "ByteDance"
    }
}

pub(super) fn is_work(agent_name: &str) -> bool {
    matches!(surface(agent_name), TraeSurface::Work(_))
}

pub(super) fn is_vscode_plugin(agent_name: &str) -> bool {
    surface(agent_name) == TraeSurface::VscodePlugin
}

pub(super) fn is_cn(agent_name: &str) -> bool {
    matches!(
        surface(agent_name),
        TraeSurface::Ide(TraeEdition::Cn) | TraeSurface::Work(TraeEdition::Cn)
    )
}

pub(super) fn state_home_dir(agent_name: &str) -> &'static str {
    if is_cn(agent_name) {
        ".trae-cn"
    } else {
        ".trae"
    }
}

pub(super) fn user_home(agent_name: &str, agent_home: &Path) -> PathBuf {
    for suffix in [
        &[state_home_dir(agent_name)][..],
        &[state_home_dir(agent_name), "work"][..],
        &[state_home_dir(agent_name), "worktrees"][..],
        &[".vscode"][..],
        &["AppData", "Roaming", "Trae"][..],
        &["AppData", "Roaming", "Trae CN"][..],
        &["AppData", "Roaming", "TraeCN"][..],
        &["AppData", "Roaming", "Trae Work"][..],
        &["AppData", "Roaming", "TRAE Work"][..],
        &["AppData", "Roaming", "TraeWork"][..],
        &["AppData", "Roaming", "TRAE SOLO"][..],
        &["AppData", "Roaming", "Trae Work CN"][..],
        &["AppData", "Roaming", "TRAE Work CN"][..],
        &["AppData", "Roaming", "TraeWorkCN"][..],
        &["AppData", "Roaming", "TRAE SOLO CN"][..],
        &["Library", "Application Support", "Trae"][..],
        &["Library", "Application Support", "Trae CN"][..],
        &["Library", "Application Support", "TraeCN"][..],
        &["Library", "Application Support", "Trae Work"][..],
        &["Library", "Application Support", "TRAE Work"][..],
        &["Library", "Application Support", "TraeWork"][..],
        &["Library", "Application Support", "TRAE SOLO"][..],
        &["Library", "Application Support", "Trae Work CN"][..],
        &["Library", "Application Support", "TRAE Work CN"][..],
        &["Library", "Application Support", "TraeWorkCN"][..],
        &["Library", "Application Support", "TRAE SOLO CN"][..],
        &[".config", "Trae"][..],
        &[".config", "Trae CN"][..],
        &[".config", "TraeCN"][..],
        &[".config", "Trae Work"][..],
        &[".config", "TRAE Work"][..],
        &[".config", "TraeWork"][..],
        &[".config", "TRAE SOLO"][..],
        &[".config", "Trae Work CN"][..],
        &[".config", "TRAE Work CN"][..],
        &[".config", "TraeWorkCN"][..],
        &[".config", "TRAE SOLO CN"][..],
    ] {
        if path_ends_with(agent_home, suffix) {
            let mut home = agent_home;
            for _ in suffix {
                home = home.parent().unwrap_or(home);
            }
            return home.to_path_buf();
        }
    }
    hidden_home_parent(agent_home)
}

pub(super) fn state_home(agent_name: &str, agent_home: &Path) -> PathBuf {
    user_home(agent_name, agent_home).join(state_home_dir(agent_name))
}

pub(super) fn ide_data_roots(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    app_data_roots(agent_name, agent_home, ide_app_names(agent_name))
}

pub(super) fn work_data_roots(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    app_data_roots(agent_name, agent_home, work_app_names(agent_name))
}

pub(super) fn vscode_extensions_home(agent_home: &Path) -> PathBuf {
    if path_ends_with(agent_home, &[".vscode"]) {
        agent_home.to_path_buf()
    } else {
        user_home("trae-vscode-plugin", agent_home).join(".vscode")
    }
}

fn app_data_roots(agent_name: &str, agent_home: &Path, app_names: &[&str]) -> Vec<PathBuf> {
    let user_home = user_home(agent_name, agent_home);
    let mut roots = Vec::new();
    if let Some(app_data) = env_path("APPDATA") {
        if app_data.starts_with(&user_home) {
            for app_name in app_names {
                roots.push(app_data.join(app_name));
            }
        }
    }
    for app_name in app_names {
        roots.extend([
            user_home.join("AppData").join("Roaming").join(app_name),
            user_home
                .join("Library")
                .join("Application Support")
                .join(app_name),
            user_home.join(".config").join(app_name),
        ]);
    }
    dedup_paths(roots)
}

pub(super) fn ide_app_names(agent_name: &str) -> &'static [&'static str] {
    if is_cn(agent_name) {
        &["Trae CN", "TraeCN"]
    } else {
        &["Trae"]
    }
}

pub(super) fn work_app_names(agent_name: &str) -> &'static [&'static str] {
    if is_cn(agent_name) {
        &[
            "Trae CN Work",
            "Trae Work CN",
            "TRAE Work CN",
            "TraeWorkCN",
            "TRAE SOLO CN",
            "TRAE SOLO_CN",
        ]
    } else {
        &["Trae Work", "TRAE Work", "TraeWork", "TRAE SOLO"]
    }
}

pub(super) fn path_ends_with(path: &Path, suffix: &[&str]) -> bool {
    let parts = path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>();
    parts.len() >= suffix.len()
        && parts[parts.len() - suffix.len()..]
            .iter()
            .zip(suffix)
            .all(|(actual, expected)| actual.eq_ignore_ascii_case(expected))
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if out.iter().any(|item: &PathBuf| item == &path) {
            continue;
        }
        out.push(path);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_aliases_to_surfaces() {
        assert_eq!(surface("trae"), TraeSurface::Ide(TraeEdition::En));
        assert_eq!(surface("trae-ide"), TraeSurface::Ide(TraeEdition::En));
        assert_eq!(surface("trae-cn"), TraeSurface::Ide(TraeEdition::Cn));
        assert_eq!(surface("trae-work"), TraeSurface::Work(TraeEdition::En));
        assert_eq!(surface("trae-solo-cn"), TraeSurface::Work(TraeEdition::Cn));
        assert_eq!(surface("trae-plugin"), TraeSurface::VscodePlugin);
    }

    #[test]
    fn resolves_user_home_from_app_roots() {
        let home = Path::new("C:/Users/me")
            .join("AppData")
            .join("Roaming")
            .join("Trae Work");

        assert_eq!(user_home("trae-work", &home), PathBuf::from("C:/Users/me"));
        assert_eq!(
            state_home("trae-work", &home),
            PathBuf::from("C:/Users/me").join(".trae")
        );
    }

    #[test]
    fn vscode_plugin_home_resolves_from_vscode_root() {
        let home = Path::new("C:/Users/me").join(".vscode");

        assert_eq!(vscode_extensions_home(&home), home);
    }
}
