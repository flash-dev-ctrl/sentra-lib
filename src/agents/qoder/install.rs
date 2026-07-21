use crate::agents::install::{
    InstallCommandPlan, Platform, curl_bash_plan, sh_plan, winget_install_plans_for_platform,
    winget_uninstall_plans_for_platform,
};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

const ID: &str = "Alibaba.Qoder";
const CONFIG: &[&str] = &["$env:USERPROFILE\\.qoder", "$env:APPDATA\\Qoder"];

pub(crate) fn install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    let mut plans = match platform {
        Platform::Windows => winget_install_plans_for_platform(platform, action, ID),
        Platform::MacOS | Platform::Linux => vec![curl_bash_plan(
            "https://qoder.com/install",
            &["--force"],
            "Qoder official installer",
        )],
    };
    if action == AgentInstallAction::Update && platform != Platform::Windows {
        plans.insert(
            0,
            InstallCommandPlan {
                program: "qodercli",
                args: vec!["update".to_string()],
                method: "qodercli update",
            },
        );
    }
    plans
}

pub(crate) fn uninstall_plans_for_platform(
    platform: Platform,
    options: AgentUninstallOptions,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_uninstall_plans_for_platform(platform, options, ID, CONFIG),
        Platform::MacOS | Platform::Linux => {
            let mut command = "set -e; rm -rf \"$HOME/.qoder/bin/qodercli\"".to_string();
            if options.delete_config {
                command.push_str("; rm -rf \"$HOME/.qoder\"");
            }
            vec![sh_plan(command, "Qoder CLI uninstall")]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_and_linux_use_qoder_checksum_verifying_installer() {
        for platform in [Platform::MacOS, Platform::Linux] {
            let plans = install_plans_for_platform(platform, AgentInstallAction::Install);
            let command = plans[0].command_line();
            assert!(command.contains("https://qoder.com/install"));
            assert!(command.contains("curl -fsSL \"$1\" -o \"$script\""));
            assert!(!command.contains("https://qoder.com/install |"));
        }
    }

    #[test]
    fn unix_uninstall_preserves_configuration_unless_requested() {
        let keep = uninstall_plans_for_platform(
            Platform::Linux,
            AgentUninstallOptions {
                delete_config: false,
            },
        )[0]
        .command_line();
        assert!(keep.contains(".qoder/bin/qodercli"));
        assert!(!keep.contains("rm -rf \"$HOME/.qoder\""));
    }

    #[test]
    fn windows_update_targets_the_winget_desktop_product_first() {
        let plans = install_plans_for_platform(Platform::Windows, AgentInstallAction::Update);

        assert_eq!(plans[0].program, "winget");
        assert!(
            !plans
                .iter()
                .any(|plan| plan.command_line() == "qodercli update")
        );
    }
}
