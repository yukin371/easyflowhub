#!/usr/bin/env bash
# build-deskflow.sh - Build DeskFlow with scriptmgr integration
#
# Usage: ./scripts/build-deskflow.sh

set -e

SCRIPTMGR_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/../scriptmgr-go"
DESKFLOW_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUNDLE_DIR="$DESKFLOW_DIR/src-tauri/target/release/bundle"

echo "=== Building DeskFlow ==="

# Step 1: Build scriptmgr.exe
echo "[1/3] Building scriptmgr.exe..."
cd "$SCRIPTMGR_DIR"
if [ ! -f "scriptmgr.exe" ] || [ "cmd/scriptmgr/main.go" -nt "scriptmgr.exe" ]; then
    go build -ldflags="-s -w" -o scriptmgr.exe ./cmd/scriptmgr
    echo "      scriptmgr.exe built"
else
    echo "      scriptmgr.exe already up-to-date"
fi

# Step 2: Copy scriptmgr.exe to deskflow resources
echo "[2/3] Copying scriptmgr.exe to deskflow..."
mkdir -p "$DESKFLOW_DIR/scriptmgr"
cp "$SCRIPTMGR_DIR/scriptmgr.exe" "$DESKFLOW_DIR/scriptmgr/"

# Step 3: Build DeskFlow
echo "[3/3] Building DeskFlow..."
cd "$DESKFLOW_DIR/deskflow-app"
bun run tauri build

echo ""
echo "=== Build Complete ==="
echo "Output: $BUNDLE_DIR"
ls -la "$BUNDLE_DIR/nsis/" 2>/dev/null || true
ls -la "$BUNDLE_DIR/msi/" 2>/dev/null || true
