#!/bin/bash
set -e

echo "=========================================================="
echo "🌐 KARACTIRAGE STUDIO — APERÇU WEB (VITE)"
echo "=========================================================="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$ROOT_DIR/apps/studio"
mkdir -p src/wasm public/pkg
cp -f "$ROOT_DIR/web/pkg/"* src/wasm/ 2>/dev/null || true
cp -f "$ROOT_DIR/web/pkg/"* public/pkg/ 2>/dev/null || true

echo "✨ Serveur démarré sur http://localhost:5173"
npm run dev
