#!/bin/bash
# Régénère les assets pixel-art KaractiRage depuis assets/art/game.px.js.
# pxl exige Node >= 22.18 pour exécuter ses sources TS en direct ; on construit
# donc dist/ une fois puis on exécute le CLI compilé (JS pur), compatible 22.14.
set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

node tools/pxl/scripts/build.js
node tools/pxl/run.js build assets/art/game.px.js --out assets/generated
