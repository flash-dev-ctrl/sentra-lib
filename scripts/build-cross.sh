#!/usr/bin/env bash
set -euo pipefail

TARGETS=(
  "x86_64-pc-windows-gnu"
  "aarch64-pc-windows-gnullvm"
  "x86_64-apple-darwin"
  "aarch64-apple-darwin"
  "x86_64-unknown-linux-musl"
  "aarch64-unknown-linux-musl"
)

usage() {
  cat <<'EOF'
Usage:
  scripts/build-cross.sh [options]

Options:
  --target <triple>   Build only selected Rust target triple. Repeat or pass comma-separated values.
  --skip-setup        Do not install Zig, cargo-zigbuild, or Rust targets.
  -h, --help          Show this help.

Targets:
  x86_64-pc-windows-gnu
  aarch64-pc-windows-gnullvm
  x86_64-apple-darwin
  aarch64-apple-darwin
  x86_64-unknown-linux-musl
  aarch64-unknown-linux-musl

Output:
  dist/<target>/sentra(.exe)
EOF
}

repo_root() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  cd "$script_dir/.." && pwd
}

has_cmd() {
  command -v "$1" >/dev/null 2>&1
}

run_as_root() {
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
  elif has_cmd sudo; then
    sudo "$@"
  else
    echo "error: setup needs root privileges. Re-run as root or install sudo." >&2
    exit 1
  fi
}

install_packages() {
  if has_cmd apt-get; then
    run_as_root apt-get update
    run_as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y curl ca-certificates build-essential pkg-config xz-utils unzip
  elif has_cmd dnf; then
    run_as_root dnf install -y curl ca-certificates gcc gcc-c++ pkgconf-pkg-config xz unzip
  elif has_cmd yum; then
    run_as_root yum install -y curl ca-certificates gcc gcc-c++ pkgconfig xz unzip
  elif has_cmd apk; then
    run_as_root apk add --no-cache curl ca-certificates build-base pkgconf xz unzip
  elif has_cmd pacman; then
    run_as_root pacman -Sy --needed --noconfirm curl ca-certificates base-devel pkgconf xz unzip
  fi
}

ensure_zig() {
  if has_cmd zig; then
    return
  fi
  install_packages
  local root tools zig_dir archive arch os zig_version url
  root="$(repo_root)"
  tools="$root/.tools"
  mkdir -p "$tools"
  zig_version="${ZIG_VERSION:-0.16.0}"
  arch="$(uname -m)"
  case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    aarch64|arm64) arch="aarch64" ;;
    *) echo "error: unsupported host architecture for Zig auto-install: $arch" >&2; exit 2 ;;
  esac
  os="$(uname -s)"
  case "$os" in
    Linux) os="linux" ;;
    Darwin) os="macos" ;;
    *) echo "error: unsupported host OS for Zig auto-install: $os" >&2; exit 2 ;;
  esac
  zig_dir="$tools/zig-${os}-${arch}-${zig_version}"
  archive="$tools/zig-${os}-${arch}-${zig_version}.tar.xz"
  url="https://ziglang.org/download/${zig_version}/zig-${os}-${arch}-${zig_version}.tar.xz"
  if [ ! -x "$zig_dir/zig" ]; then
    curl -L "$url" -o "$archive"
    tar -C "$tools" -xf "$archive"
  fi
  export PATH="$zig_dir:$PATH"
  if ! has_cmd zig; then
    echo "error: zig was not found after setup" >&2
    exit 1
  fi
}

ensure_cargo_zigbuild() {
  if ! has_cmd cargo-zigbuild; then
    cargo install cargo-zigbuild
  fi
}

target_supported() {
  local target="$1"
  for item in "${TARGETS[@]}"; do
    if [ "$item" = "$target" ]; then
      return 0
    fi
  done
  return 1
}

normalize_targets() {
  if [ "${#SELECTED_TARGETS[@]}" -eq 0 ]; then
    SELECTED_TARGETS=("${TARGETS[@]}")
  fi
  local target
  for target in "${SELECTED_TARGETS[@]}"; do
    if ! target_supported "$target"; then
      echo "error: unknown target '$target'. Use --help to list supported targets." >&2
      exit 2
    fi
  done
}

ensure_rust_targets() {
  local target
  for target in "${SELECTED_TARGETS[@]}"; do
    rustup target add "$target"
  done
}

artifact_name() {
  case "$1" in
    *windows*) printf 'sentra.exe' ;;
    *) printf 'sentra' ;;
  esac
}

SKIP_SETUP=0
SELECTED_TARGETS=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --target)
      [ "$#" -ge 2 ] || { echo "error: --target requires a value" >&2; exit 2; }
      IFS=',' read -r -a split_targets <<< "$2"
      SELECTED_TARGETS+=("${split_targets[@]}")
      shift 2
      ;;
    --skip-setup)
      SKIP_SETUP=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option '$1'" >&2
      usage >&2
      exit 2
      ;;
  esac
done

ROOT="$(repo_root)"
cd "$ROOT"
export CARGO_TARGET_DIR="$ROOT/target"
normalize_targets

if [ "$SKIP_SETUP" -eq 0 ]; then
  install_packages
  ensure_zig
  ensure_cargo_zigbuild
  ensure_rust_targets
fi

mkdir -p dist
for target in "${SELECTED_TARGETS[@]}"; do
  echo "==> Building $target"
  cargo zigbuild --release --target "$target" --bin sentra

  name="$(artifact_name "$target")"
  mkdir -p "dist/$target"
  cp "$CARGO_TARGET_DIR/$target/release/$name" "dist/$target/$name"
done

echo "Build outputs written to $ROOT/dist"
