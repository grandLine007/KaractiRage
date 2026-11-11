#!/usr/bin/env bash
# Compile le dictionnaire IA depuis le code et les fichiers curés (ai/).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo run -p engine-core --bin ai_dictionary -- --out dist/ai/current
