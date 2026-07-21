use crate::agents::install::{
    InstallCommandPlan, Platform, curl_bash_plan, sh_plan, winget_install_plans_for_platform,
    winget_uninstall_plans_for_platform,
};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

const ID: &str = "Google.AntigravityCLI";
const CONFIG: &[&str] = &["$env:USERPROFILE\\.gemini\\antigravity-cli"];

pub(crate) fn install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_install_plans_for_platform(platform, action, ID),
        Platform::MacOS | Platform::Linux => match action {
            AgentInstallAction::Install => vec![official_install_plan(platform)],
            AgentInstallAction::Update => vec![
                sh_plan(
                    "set -e; if command -v agy >/dev/null 2>&1; then agy update; else \"$HOME/.local/bin/agy\" update; fi",
                    "agy update",
                ),
                official_install_plan(platform),
            ],
            AgentInstallAction::Uninstall => Vec::new(),
        },
    }
}

fn official_install_plan(platform: Platform) -> InstallCommandPlan {
    if platform == Platform::Linux {
        linux_install_plan()
    } else {
        curl_bash_plan(
            "https://antigravity.google/cli/install.sh",
            &[],
            "Google Antigravity CLI installer",
        )
    }
}

fn linux_install_plan() -> InstallCommandPlan {
    sh_plan(
        r#"set -e
getconf GNU_LIBC_VERSION >/dev/null 2>&1 || { echo "Google Antigravity CLI does not publish a supported musl Linux build" >&2; exit 1; }
script=$(mktemp)
trap 'rm -f "$script"' EXIT
curl -fsSL "https://antigravity.google/cli/install.sh" -o "$script"
bash "$script""#,
        "Google Antigravity CLI installer",
    )
}

pub(crate) fn uninstall_plans_for_platform(
    platform: Platform,
    options: AgentUninstallOptions,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_uninstall_plans_for_platform(platform, options, ID, CONFIG),
        Platform::MacOS | Platform::Linux => {
            let mut command =
                "set -e; rm -f \"$HOME/.local/bin/agy\"; rm -rf \"$HOME/.cache/antigravity\""
                    .to_string();
            if options.delete_config {
                command.push_str("; rm -rf \"$HOME/.gemini/antigravity-cli\"");
            }
            vec![sh_plan(command, "Antigravity CLI uninstall")]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_and_linux_use_the_official_checksum_verifying_installer() {
        let macos = install_plans_for_platform(Platform::MacOS, AgentInstallAction::Install)[0]
            .command_line();
        let linux = linux_install_plan().command_line();

        assert!(macos.contains("https://antigravity.google/cli/install.sh"));
        assert!(macos.contains("curl -fsSL \"$1\" -o \"$script\""));
        assert!(linux.contains("https://antigravity.google/cli/install.sh"));
        assert!(!macos.contains("install.sh | bash"));
        assert!(!linux.contains("install.sh | bash"));
    }

    #[test]
    fn linux_rejects_unsupported_musl_before_downloading() {
        let command = linux_install_plan().command_line();

        assert!(command.contains("getconf GNU_LIBC_VERSION"));
        assert!(command.contains("does not publish a supported musl Linux build"));
    }

    #[test]
    fn unix_update_uses_the_native_cli() {
        let plans = install_plans_for_platform(Platform::Linux, AgentInstallAction::Update);
        assert!(plans[0].command_line().contains("agy update"));
        assert_eq!(plans.len(), 2);
        assert!(
            plans[1]
                .command_line()
                .contains("https://antigravity.google/cli/install.sh")
        );
    }
}
