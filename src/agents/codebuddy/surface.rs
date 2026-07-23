use std::path::{Path, PathBuf};

use crate::agents::install_status::{env_path, hidden_home_parent};

pub(crate) const CODEBUDDY_IDE_EXTENSION_ID: &str = "tencent-cloud.coding-copilot";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CodeBuddyEdition {
    En,
    Cn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CodeBuddySurface {
    Cli,
    Ide(CodeBuddyEdition),
    IdeExtension,
    Work,
    Unknown,
}

pub(super) fn surface(agent_name: &str) -> CodeBuddySurface {
    match agent_name {
        "codebuddy-cli" | "codebuddy" | "codebuddy-code" => CodeBuddySurface::Cli,
        "workbuddy" => CodeBuddySurface::Work,
        "codebuddy-ide" => CodeBuddySurface::Ide(CodeBuddyEdition::En),
        "codebuddy-cn-ide" | "codebuddy-cn" | "codebuddycn" => {
            CodeBuddySurface::Ide(CodeBuddyEdition::Cn)
        }
        "codebuddy-cli-ide" => CodeBuddySurface::IdeExtension,
        _ => CodeBuddySurface::Unknown,
    }
}

pub(super) fn title(agent_name: &str) -> &'static str {
    match surface(agent_name) {
        CodeBuddySurface::Cli => "CodeBuddy CLI",
        CodeBuddySurface::Ide(CodeBuddyEdition::En) => "CodeBuddy IDE",
        CodeBuddySurface::Ide(CodeBuddyEdition::Cn) => "CodeBuddy CN IDE",
        CodeBuddySurface::IdeExtension => "CodeBuddy IDE Extension",
        CodeBuddySurface::Work => "WorkBuddy",
        CodeBuddySurface::Unknown => "CodeBuddy",
    }
}

pub(super) fn is_cli(agent_name: &str) -> bool {
    surface(agent_name) == CodeBuddySurface::Cli
}

pub(super) fn is_ide(agent_name: &str) -> bool {
    matches!(surface(agent_name), CodeBuddySurface::Ide(_))
}

pub(super) fn is_ide_extension(agent_name: &str) -> bool {
    surface(agent_name) == CodeBuddySurface::IdeExtension
}

pub(super) fn is_work(agent_name: &str) -> bool {
    surface(agent_name) == CodeBuddySurface::Work
}

pub(super) fn is_cn(agent_name: &str) -> bool {
    matches!(
        surface(agent_name),
        CodeBuddySurface::Ide(CodeBuddyEdition::Cn)
    )
}

pub(super) fn cli_home_dir(_agent_name: &str) -> &'static str {
    ".codebuddy"
}

pub(super) fn ide_app_name(agent_name: &str) -> &'static str {
    if is_cn(agent_name) {
        "CodeBuddy CN"
    } else {
        "CodeBuddy"
    }
}

pub(super) fn ide_data_roots(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let app_name = ide_app_name(agent_name);
    let mut roots = Vec::new();
    let user_home = ide_user_home(agent_home);
    if ide_home_is_default_app_home(agent_name, agent_home)
        && let Some(app_data) = env_path("APPDATA")
        && app_data.starts_with(&user_home)
    {
        roots.push(app_data.join(app_name));
    }
    roots.extend([
        user_home.join("AppData").join("Roaming").join(app_name),
        user_home
            .join("Library")
            .join("Application Support")
            .join(app_name),
        user_home.join(".config").join(app_name),
    ]);
    dedup_paths(roots)
}

pub(super) fn ide_user_home(agent_home: &Path) -> PathBuf {
    for suffix in [
        &["AppData", "Roaming", "CodeBuddy"][..],
        &["AppData", "Roaming", "CodeBuddy CN"][..],
        &["Library", "Application Support", "CodeBuddy"][..],
        &["Library", "Application Support", "CodeBuddy CN"][..],
        &[".config", "CodeBuddy"][..],
        &[".config", "CodeBuddy CN"][..],
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

fn ide_home_is_default_app_home(agent_name: &str, agent_home: &Path) -> bool {
    ide_user_home(agent_home)
        .join("AppData")
        .join("Roaming")
        .join(ide_app_name(agent_name))
        .as_path()
        == agent_home
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
