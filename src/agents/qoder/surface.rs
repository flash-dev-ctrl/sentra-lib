use std::path::{Path, PathBuf};

use crate::agents::install_status::{env_path, hidden_home_parent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QoderEdition {
    En,
    Cn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QoderSurface {
    Cli(QoderEdition),
    Ide(QoderEdition),
    Work(QoderEdition),
}

pub(super) fn surface(agent_name: &str) -> QoderSurface {
    match agent_name {
        "qoder-cn-cli" | "qoder-cn" | "qoderclicn" | "lingma" => {
            QoderSurface::Cli(QoderEdition::Cn)
        }
        "qoder-cn-ide" => QoderSurface::Ide(QoderEdition::Cn),
        "qoder-cn-work" => QoderSurface::Work(QoderEdition::Cn),
        "qoder-ide" => QoderSurface::Ide(QoderEdition::En),
        "qoder-work" | "qoderwork" => QoderSurface::Work(QoderEdition::En),
        _ => QoderSurface::Cli(QoderEdition::En),
    }
}

pub(super) fn title(agent_name: &str) -> &'static str {
    match surface(agent_name) {
        QoderSurface::Cli(QoderEdition::En) => "Qoder CLI",
        QoderSurface::Ide(QoderEdition::En) => "Qoder IDE",
        QoderSurface::Work(QoderEdition::En) => "Qoder Work",
        QoderSurface::Cli(QoderEdition::Cn) => "Qoder CN CLI",
        QoderSurface::Ide(QoderEdition::Cn) => "Qoder CN IDE",
        QoderSurface::Work(QoderEdition::Cn) => "Qoder CN Work",
    }
}

pub(super) fn cli_command(agent_name: &str) -> &'static str {
    match surface(agent_name) {
        QoderSurface::Cli(QoderEdition::Cn) => "qoderclicn",
        _ => "qodercli",
    }
}

pub(super) fn cli_home_dir(agent_name: &str) -> &'static str {
    match surface(agent_name) {
        QoderSurface::Cli(QoderEdition::Cn) => ".qoder-cn",
        _ => ".qoder",
    }
}

pub(super) fn is_cli(agent_name: &str) -> bool {
    matches!(surface(agent_name), QoderSurface::Cli(_))
}

pub(super) fn is_ide(agent_name: &str) -> bool {
    matches!(surface(agent_name), QoderSurface::Ide(_))
}

pub(super) fn is_work(agent_name: &str) -> bool {
    matches!(surface(agent_name), QoderSurface::Work(_))
}

pub(super) fn is_cn(agent_name: &str) -> bool {
    matches!(
        surface(agent_name),
        QoderSurface::Cli(QoderEdition::Cn)
            | QoderSurface::Ide(QoderEdition::Cn)
            | QoderSurface::Work(QoderEdition::Cn)
    )
}

pub(super) fn work_data_roots(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let app_dir = if is_cn(agent_name) {
        "QoderWorkCN"
    } else {
        "QoderWork"
    };
    let mut roots = Vec::new();
    let user_home = hidden_home_parent(agent_home);
    if agent_home_is_default_work_home(agent_name, agent_home)
        && let Some(app_data) = env_path("APPDATA")
        && app_data.starts_with(&user_home)
    {
        roots.push(app_data.join(app_dir));
    }
    roots.extend([
        user_home.join("AppData").join("Roaming").join(app_dir),
        user_home
            .join("Library")
            .join("Application Support")
            .join(app_dir),
        user_home.join(".config").join(app_dir),
    ]);
    dedup_paths(roots)
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

fn agent_home_is_default_work_home(agent_name: &str, agent_home: &Path) -> bool {
    let home_dir_name = if is_cn(agent_name) {
        ".qoderwork-cn"
    } else {
        ".qoderwork"
    };
    hidden_home_parent(agent_home).join(home_dir_name).as_path() == agent_home
}
