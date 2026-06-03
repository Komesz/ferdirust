#!/bin/bash
# Bundle ferdirust with CEF runtime files
set -euo pipefail

PROFILE="${1:-release}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TARGET_DIR="$SCRIPT_DIR/target/$PROFILE"
BUNDLE_DIR="$SCRIPT_DIR/target/bundle"

# Find CEF directory from build output
CEF_DIR=$(find "$SCRIPT_DIR/target/$PROFILE/build/cef-dll-sys-"*/out/cef_linux_x86_64 -maxdepth 0 2>/dev/null | head -1)

if [ -z "$CEF_DIR" ]; then
    echo "Error: CEF directory not found. Run 'cargo build --$PROFILE' first."
    exit 1
fi

echo "CEF dir: $CEF_DIR"
echo "Bundle dir: $BUNDLE_DIR"

# Create bundle directory
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/locales"

# Copy executable
cp "$TARGET_DIR/ferdirust" "$BUNDLE_DIR/"

# Copy CEF runtime files
cp "$CEF_DIR/libcef.so" "$BUNDLE_DIR/"
cp "$CEF_DIR/libEGL.so" "$BUNDLE_DIR/"
cp "$CEF_DIR/libGLESv2.so" "$BUNDLE_DIR/"
cp "$CEF_DIR/libvk_swiftshader.so" "$BUNDLE_DIR/" 2>/dev/null || true
cp "$CEF_DIR/libvulkan.so.1" "$BUNDLE_DIR/" 2>/dev/null || true
cp "$CEF_DIR/vk_swiftshader_icd.json" "$BUNDLE_DIR/" 2>/dev/null || true
cp "$CEF_DIR/v8_context_snapshot.bin" "$BUNDLE_DIR/"
cp "$CEF_DIR/icudtl.dat" "$BUNDLE_DIR/"
cp "$CEF_DIR/chrome_100_percent.pak" "$BUNDLE_DIR/"
cp "$CEF_DIR/chrome_200_percent.pak" "$BUNDLE_DIR/"
cp "$CEF_DIR/resources.pak" "$BUNDLE_DIR/"
cp "$CEF_DIR/chrome-sandbox" "$BUNDLE_DIR/" 2>/dev/null || true

# Copy locales
cp "$CEF_DIR/locales/"*.pak "$BUNDLE_DIR/locales/"

# Copy icon
cp "$SCRIPT_DIR/resources/icon.svg" "$BUNDLE_DIR/"

echo ""
echo "Bundle created at: $BUNDLE_DIR"
echo "Run with: cd $BUNDLE_DIR && ./ferdirust"
