#!/bin/bash
set -e

echo "============================================="
echo "⚡ KARACTIRAGE — EXPORTATEUR WEB (WASM)"
echo "============================================="

# 1. Compilation de la bibliothèque cdylib pour la cible Wasm
echo "📦 Compilation de la crate engine-core en mode release (wasm32-unknown-unknown)..."
cargo build -p engine-core --target wasm32-unknown-unknown --release --lib

# 2. Génération des liaisons JS/TS via wasm-bindgen
echo "🔗 Génération des liaisons WebAssembly (web/pkg)..."
mkdir -p web/pkg
wasm-bindgen target/wasm32-unknown-unknown/release/karactirage_wasm.wasm --out-dir web/pkg --target web

echo "✨ Exportation WebAssembly prête !"
echo "Pour démarrer le serveur : python3 scripts/serve_web.py"
