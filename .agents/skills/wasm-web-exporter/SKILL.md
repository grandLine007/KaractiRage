---
name: wasm-web-exporter
description: >-
  WebAssembly (Wasm) compilation, HTML5 canvas rendering, and browser packaging for KaractiRage and KVL.
  Use when compiling crates for the wasm32-unknown-unknown target, verifying web compatibility of dependencies,
  building web-based demonstrators, or generating standalone browser builds.
---

# WebAssembly (Wasm) Exporter Skill

Ce skill détaille les procédures pour porter et exécuter **KaractiRage** et ses scripts **KVL** directement dans un navigateur Web moderne via WebAssembly (WASM) et un `<canvas>` HTML5.

---

## 1. Prérequis & Cibles Rust

* Cible installée : `wasm32-unknown-unknown` (via `rustup target add wasm32-unknown-unknown`).
* Compatibilité des dépendances :
  * `uuid` : requiert l'activation de la feature `js` (`features = ["v4", "serde", "js"]`) pour l'entropie cryptographique dans le navigateur via `crypto.getRandomValues`.
  * `serde` / `serde_json` : 100% compatibles sans modification.
  * `thiserror` : 100% compatible.

---

## 2. Procédure de Compilation

Pour vérifier l'intégrité de la compilation Wasm des bibliothèques centrales :
```bash
cargo check -p kvl-core -p kvl-runtime --target wasm32-unknown-unknown
```

Pour packager un binaire ou module Wasm avec `wasm-bindgen` :
```bash
cargo build -p kvl-runtime --target wasm32-unknown-unknown --release
```

---

## 3. Rendu sur `<canvas>` HTML5

* En environnement Web, le buffer de pixels `0x00RRGGBB` de `softbuffer` peut être transcrit directement dans un `ImageData` HTML5 en mémoire partagée (`Uint8ClampedArray`).
* La boucle d'animation est synchronisée nativement sur le taux de rafraîchissement de l'écran via `requestAnimationFrame`.
* Les événements clavier (`keydown`, `keyup`) et souris (`mousedown`, `mouseup`, `mousemove`) sont transmis directement aux abstractions `InputState`.
