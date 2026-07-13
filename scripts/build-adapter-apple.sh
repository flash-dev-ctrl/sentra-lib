#!/bin/bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="$PROJECT_DIR/dist/apple-darwin-universal"
INCLUDE_DIR="$DIST_DIR/include"
LIB_DIR="$DIST_DIR/lib"
MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-10.15}"
export MACOSX_DEPLOYMENT_TARGET

echo "=== Building sentra universal dynamic library ==="
echo "macOS deployment target: $MACOSX_DEPLOYMENT_TARGET"

ensure_target() {
    local target="$1"
    if ! rustup target list --installed | grep -qx "$target"; then
        rustup target add "$target"
    fi
}

ensure_target aarch64-apple-darwin
ensure_target x86_64-apple-darwin

# Build for arm64
echo "--- Building arm64 ---"
cargo build --release --features c-binding --target aarch64-apple-darwin

# Build for x86_64
echo "--- Building x86_64 ---"
cargo build --release --features c-binding --target x86_64-apple-darwin

# Prepare dist directory
rm -rf "$DIST_DIR"
mkdir -p "$LIB_DIR" "$INCLUDE_DIR"

# Create universal binary
echo "--- Creating universal binary ---"
lipo -create \
    "$PROJECT_DIR/target/aarch64-apple-darwin/release/libsentra_lib.dylib" \
    "$PROJECT_DIR/target/x86_64-apple-darwin/release/libsentra_lib.dylib" \
    -output "$LIB_DIR/libsentra.dylib"

install_name_tool -id @rpath/libsentra.dylib "$LIB_DIR/libsentra.dylib"

if command -v codesign >/dev/null 2>&1; then
    codesign --force --sign "${SENTRA_CODESIGN_IDENTITY:--}" "$LIB_DIR/libsentra.dylib"
fi

# Generate the header via cbindgen
echo "--- Generating C header ---"
cbindgen --config "$PROJECT_DIR/cbindgen.toml" \
    --crate sentra-lib \
    --lang C \
    --output "$INCLUDE_DIR/sentra.h" \
    "$PROJECT_DIR"

echo "=== Done ==="
echo "Library:  $LIB_DIR/libsentra.dylib"
echo "Header:   $INCLUDE_DIR/sentra.h"
echo ""
echo "Link flags (for consumer):"
echo "  -L$LIB_DIR -lsentra -rpath $LIB_DIR"
