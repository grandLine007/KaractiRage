#!/bin/bash
set -e

echo "=========================================================="
echo "🚀 KARACTIRAGE STUDIO (TAURI v2 + REACT)"
echo "=========================================================="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$ROOT_DIR/apps/studio"

echo "✨ Lancement avec Hot-Reload (Bureau macOS)..."
cargo tauri dev