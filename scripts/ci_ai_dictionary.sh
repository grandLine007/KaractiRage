#!/usr/bin/env bash
# Vérifie la non-dérive du dictionnaire IA : échec si dist/ai n'est pas à jour.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo run -p engine-core --bin ai_dictionary -- --out dist/ai/current
if ! git diff --quiet -- dist/ai; then
  echo "❌ Dictionnaire IA désynchronisé du code. Régénérez avec scripts/ai_dictionary.sh" >&2
  git diff --stat -- dist/ai >&2
  exit 1
fi
echo "✅ Dictionnaire IA à jour (dist/ai)."
