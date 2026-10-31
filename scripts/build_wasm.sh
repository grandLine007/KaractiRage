#!/bin/bash
set -e
# Build le package WASM du moteur (runtime autonome pour l'export HTML).
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "📦 Construction du moteur WASM (engine-wasm)…"
cargo build -p engine-wasm --target wasm32-unknown-unknown --release

echo "🔗 Génération de la glue JS (wasm-bindgen --target web)…"
mkdir -p dist/pkg
wasm-bindgen target/wasm32-unknown-unknown/release/engine_wasm.wasm \
  --out-dir dist/pkg \
  --target web

echo "✅ Terminé : dist/pkg/engine_wasm.js (+ engine_wasm_bg.wasm)."
echo "   Placez le dossier pkg/ à côté de votre HTML exporté (le template importe ./pkg/engine_wasm.js)."
