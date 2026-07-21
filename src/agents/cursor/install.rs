use crate::agents::install::{
    InstallCommandPlan, Platform, brew_cask_plan, brew_cask_uninstall_plan, linux_deb_rpm_plan,
    linux_package_uninstall_plan, macos_app_uninstall_plan, macos_install_app_script, sh_plan,
    winget_install_plans_for_platform, winget_uninstall_plans_for_platform,
};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

const ID: &str = "Anysphere.Cursor";
const CONFIG: &[&str] = &["$env:USERPROFILE\\.cursor", "$env:APPDATA\\Cursor"];
const MACOS_CONFIG: &[&str] = &[
    "\"$HOME/.cursor\"",
    "\"$HOME/Library/Application Support/Cursor\"",
];
const LINUX_CONFIG: &[&str] = &["\"$HOME/.cursor\"", "\"$HOME/.config/Cursor\""];

pub(crate) fn install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_install_plans_for_platform(platform, action, ID),
        Platform::MacOS => vec![brew_cask_plan("cursor", action), macos_install_plan()],
        Platform::Linux => vec![linux_deb_rpm_plan(
            "https://api2.cursor.sh/updates/download/golden/linux-x64-deb/cursor/latest",
            "https://api2.cursor.sh/updates/download/golden/linux-x64-rpm/cursor/latest",
            "https://api2.cursor.sh/updates/download/golden/linux-arm64-deb/cursor/latest",
            "https://api2.cursor.sh/updates/download/golden/linux-arm64-rpm/cursor/latest",
            "Cursor official desktop package",
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
            brew_cask_uninstall_plan("cursor", options, MACOS_CONFIG),
            macos_app_uninstall_plan("Cursor", options, MACOS_CONFIG),
        ],
        Platform::Linux => vec![linux_package_uninstall_plan(
            "cursor",
            options,
            LINUX_CONFIG,
        )],
    }
}

fn macos_install_plan() -> InstallCommandPlan {
    let mut command = r#"set -e
temp_dir=$(mktemp -d)
mkdir "$temp_dir/mount"
trap 'hdiutil detach "$temp_dir/mount" >/dev/null 2>&1 || true; rm -rf "$temp_dir"' EXIT
curl -fsSL "https://api2.cursor.sh/updates/download/golden/darwin-universal/cursor/latest" -o "$temp_dir/Cursor.dmg"
hdiutil attach -nobrowse -readonly -mountpoint "$temp_dir/mount" "$temp_dir/Cursor.dmg" >/dev/null
test -d "$temp_dir/mount/Cursor.app"
codesign --verify --deep --strict "$temp_dir/mount/Cursor.app"
spctl --assess --type execute "$temp_dir/mount/Cursor.app""#
        .to_string();
    command.push('\n');
    command.push_str(&macos_install_app_script(
        "Cursor",
        "\"$temp_dir/mount/Cursor.app\"",
    ));
    sh_plan(command, "Cursor official signed DMG")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_the_cursor_desktop_product_on_macos_and_linux() {
        let macos = install_plans_for_platform(Platform::MacOS, AgentInstallAction::Install);
        assert_eq!(macos[0].command_line(), "brew install --cask cursor");
        assert!(
            macos[1]
                .command_line()
                .contains("api2.cursor.sh/updates/download")
        );

        let linux = install_plans_for_platform(Platform::Linux, AgentInstallAction::Install);
        let command = linux[0].command_line();
        assert!(command.contains("api2.cursor.sh/updates/download/golden/linux-x64-deb"));
        assert!(!command.contains("https://cursor.com/install"));
        assert!(!command.contains("cursor-agent"));
    }

    #[test]
    fn manual_macos_install_is_updated_and_uninstalled_in_place() {
        let install = macos_install_plan().command_line();
        assert!(install.contains("/Applications/Cursor.app"));
        assert!(install.contains("sudo ditto"));

        let uninstall = uninstall_plans_for_platform(
            Platform::MacOS,
            AgentUninstallOptions {
                delete_config: false,
            },
        )[1]
        .command_line();
        assert!(uninstall.contains("$HOME/Applications/Cursor.app"));
        assert!(uninstall.contains("/Applications/Cursor.app"));
    }
}
