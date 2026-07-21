use crate::agents::install::{
    InstallCommandPlan, Platform, brew_cask_plan, brew_cask_uninstall_plan, linux_deb_rpm_plan,
    linux_package_uninstall_plan, macos_app_uninstall_plan, macos_install_app_script, sh_plan,
    winget_install_plans_for_platform, winget_uninstall_plans_for_platform,
};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

const ID: &str = "Microsoft.VisualStudioCode";
const CONFIG: &[&str] = &["$env:USERPROFILE\\.vscode", "$env:APPDATA\\Code"];
const MACOS_CONFIG: &[&str] = &[
    "\"$HOME/.vscode\"",
    "\"$HOME/Library/Application Support/Code\"",
];
const LINUX_CONFIG: &[&str] = &["\"$HOME/.vscode\"", "\"$HOME/.config/Code\""];

pub(crate) fn install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_install_plans_for_platform(platform, action, ID),
        Platform::MacOS => vec![
            brew_cask_plan("visual-studio-code", action),
            macos_install_plan(),
        ],
        Platform::Linux => vec![linux_deb_rpm_plan(
            "https://update.code.visualstudio.com/latest/linux-deb-x64/stable",
            "https://update.code.visualstudio.com/latest/linux-rpm-x64/stable",
            "https://update.code.visualstudio.com/latest/linux-deb-arm64/stable",
            "https://update.code.visualstudio.com/latest/linux-rpm-arm64/stable",
            "Microsoft VS Code package",
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
            brew_cask_uninstall_plan("visual-studio-code", options, MACOS_CONFIG),
            macos_app_uninstall_plan("Visual Studio Code", options, MACOS_CONFIG),
        ],
        Platform::Linux => vec![linux_package_uninstall_plan("code", options, LINUX_CONFIG)],
    }
}

fn macos_install_plan() -> InstallCommandPlan {
    let mut command = r#"set -e
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT
curl -fsSL "https://update.code.visualstudio.com/latest/darwin-universal/stable" -o "$temp_dir/VSCode.zip"
mkdir "$temp_dir/extracted"
ditto -x -k "$temp_dir/VSCode.zip" "$temp_dir/extracted"
test -d "$temp_dir/extracted/Visual Studio Code.app"
codesign --verify --deep --strict "$temp_dir/extracted/Visual Studio Code.app"
spctl --assess --type execute "$temp_dir/extracted/Visual Studio Code.app""#
        .to_string();
    command.push('\n');
    command.push_str(&macos_install_app_script(
        "Visual Studio Code",
        "\"$temp_dir/extracted/Visual Studio Code.app\"",
    ));
    sh_plan(command, "Microsoft VS Code signed package")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_and_linux_install_the_official_desktop_application() {
        let macos = install_plans_for_platform(Platform::MacOS, AgentInstallAction::Install);
        assert_eq!(
            macos[0].command_line(),
            "brew install --cask visual-studio-code"
        );
        assert!(
            macos[1]
                .command_line()
                .contains("update.code.visualstudio.com/latest/darwin-universal/stable")
        );

        let linux = install_plans_for_platform(Platform::Linux, AgentInstallAction::Install);
        let command = linux[0].command_line();
        assert!(
            command.contains("https://update.code.visualstudio.com/latest/linux-deb-x64/stable")
        );
        assert!(
            command.contains("https://update.code.visualstudio.com/latest/linux-rpm-arm64/stable")
        );
    }
}
