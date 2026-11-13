#!/bin/bash
set -e
# Build le package WASM du moteur (runtime autonome pour l'export HTML).
#
# Après wasm-bindgen, le script écrit dist/pkg/runtime_manifest.json : version
# du moteur + empreintes SHA-256 des artefacts. L'exporteur s'en sert pour
# rendre visible un dist/pkg périmé (pré-vol « runtime-perime ») au lieu de
# produire silencieusement un jeu avec un ancien moteur.
# Le manifeste ne contient AUCUN horodatage : deux builds identiques donnent un
# fichier identique au bit près.
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PKG_DIR="dist/pkg"
WASM_FILE="engine_wasm_bg.wasm"
GLUE_FILE="engine_wasm.js"

echo "📦 Construction du moteur WASM (engine-wasm)…"
cargo build -p engine-wasm --target wasm32-unknown-unknown --release

echo "🔗 Génération de la glue JS (wasm-bindgen --target web)…"
mkdir -p "$PKG_DIR"
wasm-bindgen target/wasm32-unknown-unknown/release/engine_wasm.wasm \
  --out-dir "$PKG_DIR" \
  --target web

# --- Empreintes SHA-256 (shasum sur macOS, sha256sum sur Linux) --------------
if command -v shasum >/dev/null 2>&1; then
  SHA256_CMD="shasum -a 256"
elif command -v sha256sum >/dev/null 2>&1; then
  SHA256_CMD="sha256sum"
else
  echo "❌ ni shasum ni sha256sum n'est disponible : manifeste impossible." >&2
  exit 1
fi

WASM_SHA="$($SHA256_CMD "$PKG_DIR/$WASM_FILE" | cut -d' ' -f1)"
GLUE_SHA="$($SHA256_CMD "$PKG_DIR/$GLUE_FILE" | cut -d' ' -f1)"

# --- Version du moteur -------------------------------------------------------
# Source de vérité : `cargo metadata --no-deps`, qui ne touche pas au réseau.
ENGINE_VERSION="$(cargo metadata --no-deps -q --format-version 1 2>/dev/null | python3 -c '
import json, sys
paquets = json.load(sys.stdin)["packages"]
print(next((p["version"] for p in paquets if p["name"] == "engine-core"), ""))
' 2>/dev/null || true)"

# Repli sans python3 : version de l'espace de travail dans le Cargo.toml racine.
if [ -z "$ENGINE_VERSION" ]; then
  ENGINE_VERSION="$(awk '
    /^\[workspace\.package\]/ { dans = 1; next }
    dans && /^version *=/ { gsub(/[" ]/, "", $3); print $3; exit }
  ' Cargo.toml)"
fi
if [ -z "$ENGINE_VERSION" ]; then
  echo "❌ version du moteur introuvable : manifeste impossible." >&2
  exit 1
fi

# Ordre des clés fixe et aucune date : le fichier est reproductible.
cat > "$PKG_DIR/runtime_manifest.json" <<EOF
{
  "engine_version": "$ENGINE_VERSION",
  "wasm_sha256": "$WASM_SHA",
  "glue_sha256": "$GLUE_SHA",
  "target": "wasm32-unknown-unknown"
}
EOF

echo "✅ Terminé : $PKG_DIR/engine_wasm.js (+ $WASM_FILE)."
echo "   Manifeste : $PKG_DIR/runtime_manifest.json (moteur $ENGINE_VERSION, wasm sha256 ${WASM_SHA:0:12}…)."
echo "   Les exports HTML embarquent ces artefacts automatiquement (dist/pkg)."
echo "   Pour un binaire packagé hors dépôt, définissez KARACTI_RUNTIME_DIR=chemin/vers/pkg."
