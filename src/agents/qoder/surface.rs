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
