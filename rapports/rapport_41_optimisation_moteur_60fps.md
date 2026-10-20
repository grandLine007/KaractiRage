# RAPPORT D'INTERVENTION N°41 : OPTIMISATION DU MOTEUR — TRANSPORT MONO-PASSE, TÉLÉMÉTRIE, ZÉRO-ALLOCATION, RASTERIZER, PAS FIXE

> **Date** : 20 Octobre 2026
> **Auteur** : Cline (assistant IA)
> **Branche / Cible** : `feat/tauri-react-studio` — `crates/engine-core` (`native_api.rs`, `game.rs`, `renderer.rs`, `benches/`), `apps/studio` (`lib.rs`, `GameViewport.tsx`), `Cargo.toml`
> **Statut** : ✅ **SUCCÈS — 144 tests engine-core + tests kvl (0 échec), `cargo check --workspace` propre, `npm run build` + `oxlint` propres**

---

## 1. Contexte & objectif

Le moteur (rendu logiciel 1200×800, 60 FPS) a été audité pour garantir la conformité au profil
*Low-End* du cahier des charges. L'audit a révélé un gaspillage de transport (conversion + sérialisation
sur 3,84 Mo **par frame**), une télémétrie partielle qui faussait l'adaptation de qualité, des allocations
par frame (clone d'entités et de graphes KVL), un rasterizer sous-optimal et un plafond silencieux de
16 candidats de collision.

Objectif : **chiffrer** chaque optimisation (pas de supposition), corriger les défauts de mesure et
réduire le temps de frame CPU, en gardant 100 % des tests verts à chaque étape.

## 2. Protocole de mesure

* Nouveau harnais de benchmark **sans dépendance** : `crates/engine-core/benches/engine_bench.rs`
  (`[[bench]] harness = false`, médianes sur `Instant`, aucune dépendance réseau ajoutée).
* Mesure par phase : `update` / `render` / `transport` / `TOTAL`, plus micro-benches
  (`draw_circle`, `blend_pixel`, `draw_text_mono`).
* Exécution : `cargo bench -p engine-core --bench engine_bench`.
* **Profils de build ajoutés** (`Cargo.toml`) : `[profile.dev] opt-level = 2`,
  `[profile.dev.package."*"] opt-level = 3`, `[profile.release] lto = "thin", codegen-units = 1`.
  Le Studio (`cargo tauri dev`) n'est plus compilé en `opt-level = 0`.

## 3. Réalisations (par étape)

### 3.1 Transport RGBA mono-passe (`native_api.rs`, `lib.rs`)
Suppression de `rgba_buffer`, `rgba_bytes()` et `convert_xrgb_to_rgba`. Nouveau `frame_bytes()` qui
échange les canaux R/B **et** écrit les 4 octets du pixel en **une seule passe** (`spare_capacity_mut`
→ `align_to_mut::<u32>` → `set_len`). Élimine 2 passes complètes + 960 000 `to_le_bytes`/frame.

### 3.2 Télémétrie honnête & correction de l'adaptation (`native_api.rs`, `game.rs`, `lib.rs`)
Le chrono ne mesurait que le début d'`update()` → l'`AdaptationManager` ne dégradait jamais le profil.
Nouveau `FrameStats { update_us, render_us, encode_us, total_us }` mesuré dans `render_frame`, transmis à
`evaluate_adaptation(total_us)`, exposé par la commande unique `engine_telemetry()`.

### 3.3 Zéro allocation par frame (`game.rs`)
* Suppression du `clone()` complet du patch IA (`Vec<Entity>` + `Graph` KVL) à chaque frame → test de
  présence + capture d'un simple `id`.
* `render_order` et `hit_events` deviennent des tampons réutilisables sur `GameState`.
* `active_tooltip` rendu par référence (plus de clone de 3 `String`).
* Throttling audio `BlockPush` (cooldown 150 ms) : plus d'empilement de voix pendant une poussée.

### 3.4 Rasterizer (`renderer.rs`)
* `draw_circle` → remplissage par scanlines (une écriture contiguë par ligne), pixel-identique
  (validé par test bruteforce `test_draw_circle_matches_bruteforce`).
* `blend_pixel` → division par 255 remplacée par `div255(v) = (v + 1 + (v >> 8)) >> 8`, **exacte**
  sur l'intervalle de blending [0, 65025] (validé exhaustivement `test_div255_exact_for_blend_range`).

### 3.5 Suppression du plafond de 16 candidats de collision (`game.rs`)
`cand_buf[16]` tronquait silencieusement les candidats (risque de tunneling en zone dense). Remplacé
par `collision_candidates: Vec<SpatialItem>` réutilisé. Test de non-régression
`test_collision_resolves_more_than_16_candidates` (20 murs empilés).

### 3.6 Frontend (`GameViewport.tsx`)
Télémétrie groupée (1 IPC/10 frames au lieu de 4) et coalescence de la souris (1 IPC/frame au lieu de
1 par événement `mousemove`).

### 3.7 Pas de simulation fixe (`native_api.rs`)
Accumulateur à **120 Hz** (`SIM_DT = 1/120`), plafonné à 4 sous-pas (anti « spirale de la mort »),
entrées transitoires consommées une seule fois par frame. Déterminisme indépendant du taux de
rafraîchissement. Test `test_fixed_timestep_clamps_substeps`.

## 4. Résultats mesurés

Benchmark release, scène `level_01` (39 entités), médianes :

| Métrique | Avant | Après | Gain |
|---|---|---|---|
| Transport (convert + encode) | 510 µs | **64 µs** | **×8** |
| TOTAL frame | 799 µs | **322 µs** | **×2,5** |
| `draw_circle` (r=42) | 5,7 µs | **0,6 µs** | **×9,5** |
| `blend_pixel` (alpha 128) | 0,014 µs | **0,003 µs** | **×4,7** |
| Marge 60 FPS | 15 867 µs | **16 344 µs** | 98,1 % du budget |

Cibles du profil *Low-End* : update ≤ 200 µs ✅ (1,2 µs), raster ≤ 2 500 µs ✅ (257 µs),
total ≤ 16 666 µs ✅ (322 µs).

## 5. Limites & suite

* Les ~25 `format!` par frame dans `render()` (petites allocations de `String`) n'ont pas été remplacés :
  impact faible comparé aux clones d'entités/graphe déjà supprimés.
* L'interpolation visuelle (`interpolated_x/y`, déjà présente) reste non câblée : le rendu affiche le
  dernier état simulé.
* **Étapes restantes (suivi séparé)** :
  * *Étape 10* — boucle Rust + `tauri::ipc::Channel` binaire (le payload brut 3,84 Mo/frame reste
    optimisable, mais n'est plus un goulot : transport à 64 µs).
  * *Étape 11* — diff par tuiles 64×64 + câblage de `QualityProfile::render_scale()` (mort aujourd'hui).
  * *Étape 12* — découpage de `game.rs` (12 557 lignes) en modules `sim/` et `ui/`.

## 6. Artefacts livrés

* Modifié : `Cargo.toml` (profils), `crates/engine-core/Cargo.toml` (`[[bench]]`),
  `crates/engine-core/src/game.rs`, `crates/engine-core/src/renderer.rs`,
  `apps/studio/src-tauri/src/lib.rs`, `apps/studio/src/components/GameViewport.tsx`.
* Créé : `crates/engine-core/src/native_api.rs`, `crates/engine-core/benches/engine_bench.rs`, ce rapport.

## 7. Validation

* `cargo test --workspace` : **100 % vert** (144 tests engine-core + 13 tests kvl, 0 échec, 1 ignoré).
* `cargo check --workspace` : sans erreur ni warning.
* `npm run build` (tsc + vite) : OK. `oxlint` : 0 erreur.
* Tests ajoutés : `test_swizzle_xrgb_to_rgba`, `test_frame_bytes_encodes_rgba_order`,
  `test_div255_exact_for_blend_range`, `test_draw_circle_matches_bruteforce`,
  `test_collision_resolves_more_than_16_candidates`, `test_fixed_timestep_clamps_substeps`.
