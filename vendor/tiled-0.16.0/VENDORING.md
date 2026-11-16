# Vendoring de `tiled` 0.16.0

Source : https://static.crates.io/crates/tiled/tiled-0.16.0.crate (MIT, LICENSE incluse).

## Différences avec l'amont

`Cargo.toml` a été **allégé** pour permettre une compilation hors ligne (crates.io
inaccessible sur cette machine) :

- dépendances optionnelles retirées : `ggez`, `sfml`, `zstd`, `regex`, `serde`,
  `serde_json`, `serde_regex` ;
- features retirées : `wasm`, `world`, `example-ggez`, `example-sfml` ;
- `default = []` (au lieu de `["zstd"]`).

Conséquences : les cartes Tiled compressées en **zstd** ne sont pas lisibles ; les
features `world` et les exemples non plus. Le code de la bibliothèque n'a pas été modifié.

Pour revenir à l'amont : remplacer ce dossier par l'archive officielle avec un réseau
disponible, et retirer `[patch.crates-io] quick-xml` du `Cargo.toml` racine.
