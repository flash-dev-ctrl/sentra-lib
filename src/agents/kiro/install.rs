use crate::agents::install::{
    InstallCommandPlan, Platform, brew_cask_plan, brew_cask_uninstall_plan,
    linux_package_uninstall_plan, macos_app_uninstall_plan, macos_install_app_script, sh_plan,
    winget_install_plans_for_platform, winget_uninstall_plans_for_platform,
};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

const ID: &str = "Amazon.Kiro";
const CONFIG: &[&str] = &["$env:USERPROFILE\\.kiro", "$env:APPDATA\\Kiro"];
const MACOS_CONFIG: &[&str] = &[
    "\"$HOME/.kiro\"",
    "\"$HOME/Library/Application Support/Kiro\"",
];
const LINUX_CONFIG: &[&str] = &["\"$HOME/.kiro\"", "\"$HOME/.config/Kiro\""];

pub(crate) fn install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_install_plans_for_platform(platform, action, ID),
        Platform::MacOS => vec![brew_cask_plan("kiro", action), macos_install_plan()],
        Platform::Linux => vec![linux_deb_plan(), linux_tar_plan()],
    }
}

pub(crate) fn uninstall_plans_for_platform(
    platform: Platform,
    options: AgentUninstallOptions,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_uninstall_plans_for_platform(platform, options, ID, CONFIG),
        Platform::MacOS => vec![
            brew_cask_uninstall_plan("kiro", options, MACOS_CONFIG),
            macos_app_uninstall_plan("Kiro", options, MACOS_CONFIG),
        ],
        Platform::Linux => vec![
            linux_package_uninstall_plan("kiro", options, LINUX_CONFIG),
            linux_tar_uninstall_plan(options),
        ],
    }
}

fn macos_install_plan() -> InstallCommandPlan {
    let mut command = r#"set -e
case "$(uname -m)" in arm64|aarch64) metadata=metadata-darwin-arm64-stable.json; expected=darwin-arm64 ;; x86_64|amd64) metadata=metadata-darwin-x64-stable.json; expected=darwin-x64 ;; *) echo "unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;; esac
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT
curl -fsSL "https://prod.download.desktop.kiro.dev/stable/$metadata" -o "$temp_dir/release.json"
url=$(plutil -extract releases.0.updateTo.url raw -o - "$temp_dir/release.json")
case "$url" in https://prod.download.desktop.kiro.dev/releases/stable/$expected/signed/*/kiro-ide-*-stable-$expected.zip) ;; *) echo "Kiro metadata returned an untrusted download URL" >&2; exit 1 ;; esac
curl -fsSL "$url" -o "$temp_dir/Kiro.zip"
mkdir "$temp_dir/extracted"
ditto -x -k "$temp_dir/Kiro.zip" "$temp_dir/extracted"
app=$(find "$temp_dir/extracted" -maxdepth 2 -type d -name Kiro.app -print -quit)
test -n "$app"
codesign --verify --deep --strict "$app"
spctl --assess --type execute "$app""#
        .to_string();
    command.push('\n');
    command.push_str(&macos_install_app_script("Kiro", "\"$app\""));
    sh_plan(command, "Kiro official signed package")
}

fn linux_deb_plan() -> InstallCommandPlan {
    sh_plan(
        r#"set -e
case "$(uname -m)" in x86_64|amd64) ;; *) echo "Kiro IDE for Linux is only published for x86_64" >&2; exit 1 ;; esac
command -v apt-get >/dev/null 2>&1 || { echo "Kiro IDE requires a supported deb-based Linux distribution" >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "python3 is required to read the official Kiro release metadata" >&2; exit 1; }
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT
curl -fsSL "https://prod.download.desktop.kiro.dev/stable/metadata-linux-x64-deb-stable.json" -o "$temp_dir/release.json"
url=$(python3 - "$temp_dir/release.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as release_file:
    releases = json.load(release_file)["releases"]
print(next(item["updateTo"]["url"] for item in releases if item["updateTo"]["url"].endswith(".deb")))
PY
)
case "$url" in https://prod.download.desktop.kiro.dev/releases/stable/linux-x64/signed/*/deb/kiro-ide-*-stable-linux-x64.deb) ;; *) echo "Kiro metadata returned an untrusted download URL" >&2; exit 1 ;; esac
curl -fsSL "$url" -o "$temp_dir/kiro.deb"
if [ "$(id -u)" -eq 0 ]; then elevate=; elif command -v sudo >/dev/null 2>&1; then elevate=sudo; else echo "root privileges or sudo are required to install Kiro IDE" >&2; exit 1; fi
$elevate apt-get install -y "$temp_dir/kiro.deb""#,
        "Kiro official desktop package",
    )
}

fn linux_tar_plan() -> InstallCommandPlan {
    sh_plan(
        r#"set -e
case "$(uname -m)" in x86_64|amd64) ;; *) echo "Kiro IDE for Linux is only published for x86_64" >&2; exit 1 ;; esac
command -v python3 >/dev/null 2>&1 || { echo "python3 is required to read the official Kiro release metadata" >&2; exit 1; }
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT
curl -fsSL "https://prod.download.desktop.kiro.dev/stable/metadata-linux-x64-stable.json" -o "$temp_dir/release.json"
url=$(python3 - "$temp_dir/release.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as release_file:
    releases = json.load(release_file)["releases"]
print(next(item["updateTo"]["url"] for item in releases if item["updateTo"]["url"].endswith(".tar.gz")))
PY
)
case "$url" in https://prod.download.desktop.kiro.dev/releases/stable/linux-x64/signed/*/tar/kiro-ide-*-stable-linux-x64.tar.gz) ;; *) echo "Kiro metadata returned an untrusted download URL" >&2; exit 1 ;; esac
curl -fsSL "$url" -o "$temp_dir/kiro.tar.gz"
if tar -tzf "$temp_dir/kiro.tar.gz" | grep -Eq '(^/|(^|/)\.\.(/|$))'; then echo "Kiro archive contains an unsafe path" >&2; exit 1; fi
mkdir "$temp_dir/extracted"
tar -xzf "$temp_dir/kiro.tar.gz" -C "$temp_dir/extracted"
executable=$(find "$temp_dir/extracted" -maxdepth 4 -type f -iname kiro -perm -u+x -print -quit)
test -n "$executable"
app_dir=$(dirname "$executable")
binary=$(basename "$executable")
mkdir -p "$HOME/.local/share" "$HOME/.local/bin"
rm -rf "$HOME/.local/share/kiro"
mv "$app_dir" "$HOME/.local/share/kiro"
ln -sfn "$HOME/.local/share/kiro/$binary" "$HOME/.local/bin/kiro""#,
        "Kiro official universal Linux package",
    )
}

fn linux_tar_uninstall_plan(options: AgentUninstallOptions) -> InstallCommandPlan {
    let mut command =
        "set -e; rm -f \"$HOME/.local/bin/kiro\"; rm -rf \"$HOME/.local/share/kiro\"".to_string();
    if options.delete_config {
        command.push_str("; rm -rf \"$HOME/.kiro\" \"$HOME/.config/Kiro\"");
    }
    sh_plan(command, "Kiro universal package uninstall")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_full_kiro_ide_instead_of_cli_shim() {
        let macos = install_plans_for_platform(Platform::MacOS, AgentInstallAction::Install);
        assert_eq!(macos[0].command_line(), "brew install --cask kiro");
        assert!(
            macos[1]
                .command_line()
                .contains("metadata-darwin-arm64-stable.json")
        );

        let linux = install_plans_for_platform(Platform::Linux, AgentInstallAction::Install);
        assert!(
            linux[0]
                .command_line()
                .contains("metadata-linux-x64-deb-stable.json")
        );
        assert!(
            linux[1]
                .command_line()
                .contains("metadata-linux-x64-stable.json")
        );
        assert!(linux[1].command_line().contains("stable-linux-x64.tar.gz"));
        assert!(!linux[1].command_line().contains("apt-get"));
        assert!(
            !linux
                .iter()
                .any(|plan| plan.command_line().contains("cli.kiro.dev/install"))
        );
    }
}
