use crate::agents::install::{
    InstallCommandPlan, Platform, brew_cask_plan, brew_cask_uninstall_plan,
    linux_package_uninstall_plan, macos_app_uninstall_plan, macos_install_app_script, sh_plan,
    winget_install_plans_for_platform, winget_uninstall_plans_for_platform,
};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

const ID: &str = "ByteDance.Trae";
const CONFIG: &[&str] = &["$env:USERPROFILE\\.trae", "$env:APPDATA\\Trae"];
const MACOS_CONFIG: &[&str] = &[
    "\"$HOME/.trae\"",
    "\"$HOME/Library/Application Support/Trae\"",
];
const LINUX_CONFIG: &[&str] = &["\"$HOME/.trae\"", "\"$HOME/.config/Trae\""];

pub(crate) fn install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_install_plans_for_platform(platform, action, ID),
        Platform::MacOS => vec![brew_cask_plan("trae", action), macos_install_plan()],
        Platform::Linux => vec![linux_install_plan()],
    }
}

pub(crate) fn uninstall_plans_for_platform(
    platform: Platform,
    options: AgentUninstallOptions,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_uninstall_plans_for_platform(platform, options, ID, CONFIG),
        Platform::MacOS => vec![
            brew_cask_uninstall_plan("trae", options, MACOS_CONFIG),
            macos_app_uninstall_plan("Trae", options, MACOS_CONFIG),
        ],
        Platform::Linux => vec![linux_package_uninstall_plan("trae", options, LINUX_CONFIG)],
    }
}

fn macos_install_plan() -> InstallCommandPlan {
    let mut command = r#"set -e
case "$(uname -m)" in arm64|aarch64) download_key=apple ;; x86_64|amd64) download_key=intel ;; *) echo "unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;; esac
temp_dir=$(mktemp -d)
mkdir "$temp_dir/mount"
trap 'hdiutil detach "$temp_dir/mount" >/dev/null 2>&1 || true; rm -rf "$temp_dir"' EXIT
curl -fsSL "https://icube-normal.trae.ai/icube/api/v1/native/version/trae/latest" -o "$temp_dir/release.json"
index=0
url=
while region=$(plutil -extract "data.manifest.darwin.download.$index.region" raw -o - "$temp_dir/release.json" 2>/dev/null); do
  if [ "$region" = sg ]; then url=$(plutil -extract "data.manifest.darwin.download.$index.$download_key" raw -o - "$temp_dir/release.json"); break; fi
  index=$((index + 1))
done
test -n "$url"
case "$url" in https://lf-cdn.trae.ai/obj/*/darwin/Trae-darwin-*.dmg) ;; *) echo "TRAE manifest returned an untrusted download URL" >&2; exit 1 ;; esac
curl -fsSL "$url" -o "$temp_dir/Trae.dmg"
hdiutil attach -nobrowse -readonly -mountpoint "$temp_dir/mount" "$temp_dir/Trae.dmg" >/dev/null
app=$(find "$temp_dir/mount" -maxdepth 2 -type d -name Trae.app -print -quit)
test -n "$app"
codesign --verify --deep --strict "$app"
spctl --assess --type execute "$app""#
        .to_string();
    command.push('\n');
    command.push_str(&macos_install_app_script("Trae", "\"$app\""));
    sh_plan(command, "TRAE official signed DMG")
}

fn linux_install_plan() -> InstallCommandPlan {
    sh_plan(
        r#"set -e
command -v python3 >/dev/null 2>&1 || { echo "python3 is required to read the official TRAE release manifest" >&2; exit 1; }
case "$(uname -m)" in x86_64|amd64) deb_key=x64.deb; rpm_key=x64.rpm ;; aarch64|arm64) deb_key=arm64.deb; rpm_key=arm64.rpm ;; *) echo "unsupported Linux architecture: $(uname -m)" >&2; exit 1 ;; esac
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT
curl -fsSL "https://icube-normal.trae.ai/icube/api/v1/native/version/trae/latest" -o "$temp_dir/release.json"
extract_url() {
  python3 - "$temp_dir/release.json" "$1" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as release_file:
    downloads = json.load(release_file)["data"]["manifest"]["linux"]["download"]
release = next(item for item in downloads if item.get("region") == "sg")
print(release[sys.argv[2]])
PY
}
deb_url=$(extract_url "$deb_key")
rpm_url=$(extract_url "$rpm_key")
for url in "$deb_url" "$rpm_url"; do
  case "$url" in https://lf-cdn.trae.ai/obj/*) ;; *) echo "TRAE manifest returned an untrusted download URL" >&2; exit 1 ;; esac
done
if [ "$(id -u)" -eq 0 ]; then elevate=; elif command -v sudo >/dev/null 2>&1; then elevate=sudo; else echo "root privileges or sudo are required to install TRAE" >&2; exit 1; fi
if command -v apt-get >/dev/null 2>&1; then
  curl -fsSL "$deb_url" -o "$temp_dir/trae.deb"
  $elevate apt-get install -y "$temp_dir/trae.deb"
elif command -v dnf >/dev/null 2>&1; then
  curl -fsSL "$rpm_url" -o "$temp_dir/trae.rpm"
  $elevate dnf install -y "$temp_dir/trae.rpm"
elif command -v yum >/dev/null 2>&1; then
  curl -fsSL "$rpm_url" -o "$temp_dir/trae.rpm"
  $elevate yum install -y "$temp_dir/trae.rpm"
else
  echo "a supported deb or rpm package manager is required" >&2
  exit 1
fi"#,
        "TRAE official desktop package",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_uses_verified_homebrew_cask() {
        let plans = install_plans_for_platform(Platform::MacOS, AgentInstallAction::Install);
        assert_eq!(plans[0].command_line(), "brew install --cask trae");
        assert!(
            plans[1]
                .command_line()
                .contains("data.manifest.darwin.download")
        );
        assert!(plans[1].command_line().contains("codesign --verify"));
    }

    #[test]
    fn linux_uses_the_vendor_manifest_and_desktop_packages() {
        let plans = install_plans_for_platform(Platform::Linux, AgentInstallAction::Install);
        let command = plans[0].command_line();
        assert!(command.contains("icube-normal.trae.ai/icube/api/v1/native/version/trae/latest"));
        assert!(command.contains("https://lf-cdn.trae.ai/obj/"));
        assert!(command.contains("x64.deb"));
        assert!(command.contains("arm64.rpm"));
        assert!(!command.contains("trae-cli"));
    }
}
