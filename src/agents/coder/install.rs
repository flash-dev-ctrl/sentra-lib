use crate::agents::install::{
    InstallCommandPlan, Platform, brew_plan, curl_bash_plan, linux_package_uninstall_plan, sh_plan,
    winget_install_plans_for_platform, winget_uninstall_plans_for_platform,
};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

const ID: &str = "Coder.Coder";
const CONFIG: &[&str] = &[
    "$env:CODER_CONFIG_DIR",
    "$env:USERPROFILE\\.config\\coderv2",
];
const UNIX_CONFIG: &[&str] = &[
    "\"${CODER_CONFIG_DIR:-$HOME/.config/coderv2}\"",
    "\"$HOME/.cache/coder\"",
];

pub(crate) fn install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_install_plans_for_platform(platform, action, ID),
        Platform::MacOS => vec![
            brew_plan("coder", action),
            curl_bash_plan(
                "https://coder.com/install.sh",
                &["--stable"],
                "Coder official installer",
            ),
        ],
        Platform::Linux => vec![curl_bash_plan(
            "https://coder.com/install.sh",
            &["--stable"],
            "Coder official installer",
        )],
    }
}

pub(crate) fn uninstall_plans_for_platform(
    platform: Platform,
    options: AgentUninstallOptions,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_uninstall_plans_for_platform(platform, options, ID, CONFIG),
        Platform::MacOS => vec![
            brew_uninstall_plan(options),
            standalone_uninstall_plan(options, UNIX_CONFIG),
        ],
        Platform::Linux => vec![
            linux_package_uninstall_plan("coder", options, UNIX_CONFIG),
            standalone_uninstall_plan(options, UNIX_CONFIG),
        ],
    }
}

fn brew_uninstall_plan(options: AgentUninstallOptions) -> InstallCommandPlan {
    let mut command = "set -e; brew uninstall coder".to_string();
    if options.delete_config {
        command.push_str(
            "; rm -rf \"${CODER_CONFIG_DIR:-$HOME/.config/coderv2}\" \"$HOME/.cache/coder\"",
        );
    }
    sh_plan(command, "Homebrew")
}

fn standalone_uninstall_plan(
    options: AgentUninstallOptions,
    config_paths: &[&str],
) -> InstallCommandPlan {
    let mut command = "set -e; rm -f \"$HOME/.local/bin/coder\"; if [ -e /usr/local/bin/coder ]; then if [ -w /usr/local/bin/coder ] || [ -w /usr/local/bin ]; then rm -f /usr/local/bin/coder; elif command -v sudo >/dev/null 2>&1; then sudo rm -f /usr/local/bin/coder; else exit 1; fi; fi".to_string();
    if options.delete_config && !config_paths.is_empty() {
        command.push_str("; rm -rf ");
        command.push_str(&config_paths.join(" "));
    }
    sh_plan(command, "Coder standalone uninstall")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_prefers_formula_and_linux_uses_safe_official_script() {
        let macos = install_plans_for_platform(Platform::MacOS, AgentInstallAction::Install);
        assert_eq!(macos[0].command_line(), "brew install coder");

        let linux = install_plans_for_platform(Platform::Linux, AgentInstallAction::Install);
        let command = linux[0].command_line();
        assert!(command.contains("https://coder.com/install.sh"));
        assert!(command.contains("curl -fsSL \"$1\" -o \"$script\""));
        assert!(!command.contains("curl -L https://coder.com/install.sh |"));
    }

    #[test]
    fn forced_uninstall_uses_coder_config_dir_on_unix() {
        let command = standalone_uninstall_plan(
            AgentUninstallOptions {
                delete_config: true,
            },
            UNIX_CONFIG,
        )
        .command_line();

        assert!(command.contains("${CODER_CONFIG_DIR:-$HOME/.config/coderv2}"));
        assert!(!command.contains("Library/Application Support/coderv2"));
    }
}
