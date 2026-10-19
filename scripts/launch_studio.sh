#!/bin/bash
set -e

echo "=========================================================="
echo "🚀 KARACTIRAGE STUDIO (TAURI v2 + REACT)"
echo "=========================================================="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$ROOT_DIR/apps/studio"

# Synchronisation automatique du package WebAssembly le plus récent
mkdir -p src/wasm public/pkg
cp -f "$ROOT_DIR/web/pkg/"* src/wasm/ 2>/dev/null || true
cp -f "$ROOT_DIR/web/pkg/"* public/pkg/ 2>/dev/null || true

echo "✨ Lancement avec Hot-Reload (Bureau macOS)..."
cargo tauri dev