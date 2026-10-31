# RAPPORT D'INTERVENTION N°42 : INTÉGRATION SVG AU MOTEUR 2D — DÉCOR, PERSONNAGES & ANIMATION

> **Date** : 20 Octobre 2026
> **Auteur** : Cline (assistant IA)
> **Branche / Cible** : `feat/tauri-react-studio` — `crates/engine-core` (`vector.rs`, `entity.rs`, `renderer.rs`, `game.rs`, `benches/`), `crates/kvl-runtime` (`interpreter.rs`, `lib.rs`), `assets/sprites/`, `assets/scenes/`
> **Statut** : ✅ **SUCCÈS — 176 tests (0 échec), `cargo check --workspace` + wasm32 propres, `npm run build` + `oxlint` propres**

---

## 1. Contexte & objectif

Intégrer des assets **SVG** au moteur 2D pour le **décor**, les **personnages** et leur
**animation**, en restant strictement conforme aux deux documents de vision
(`VISION_CIBLE_VERSION_FINALE.md`, `KaractiRage_vision_technique_etude_2026.docx.md`) :
Low-End First, binaire < 60 Mo, formats ouverts, licences permissives, **zéro allocation par frame**,
IA assistive (pas autonome).

## 2. Recherche & décision

| Solution | Licence | Rendu | Animation | Verdict |
|---|---|---|---|---|
| **`usvg` + `resvg`/`tiny-skia`** | Apache-2.0 **ou** MIT / BSD-3 | **CPU pur** | ❌ statique | ✅ **Retenu** |
| `vello` | Apache/MIT | GPU ou CPU | — | ⚠️ Surcoût (parseur SVG requis en plus) |
| `rsvg`/librsvg | MIT binding / **LGPL-GPL** | CPU | ✅ SMIL | ❌ Dépendances C (glib, cairo, pango) |
| nanosvg / LunaSVG | — | CPU | ❌ | ❌ Immatures |

**Décision** : `resvg 0.48` avec **`default-features = false`** (pas de `text`, `system-fonts`,
`raster-images`, `svgz`) → 100 % Rust, sans dépendance système C, binaire minimal, compilable **wasm32**.

## 3. Verrou technique : absence de SMIL

`usvg`/`resvg` ne supportent **pas** les animations SMIL (`<animate>`, `<animateTransform>`) —
parsing statique uniquement. L'animation est donc portée par une **couche moteur** :

1. **Rig squelettique 2D** : chaque `<g id="...">` devient un membre rastérisé isolément
   (`Tree::node_by_id` + `Group::abs_layer_bounding_box` + `resvg::render_node`), composé à l'exécution
   par transform (rotation/pivot/translation/échelle) — **aucune re-rasterisation par frame**.
2. **Clips JSON ouverts** (`assets/sprites/*.anim.json`) : keyframes par membre, interpolation linéaire.
3. **Pilotage KVL** : `PlayAnimation2D:<clip>`, `PlayAnimation2DOn:<tag>:<clip>`, `StopAnimation2D`.

## 4. Architecture retenue

```
assets/sprites/*.svg  ──import──►  usvg::Tree  ──► resvg (CPU) ──► bitmap RGBA prémultiplié
assets/sprites/*.anim.json ──────────► clips keyframes (id → index, résolus à l'import)
        │
        ▼  (Arc partagés, cache par asset)
Entity.vector_sprite { image, rig, clip, time, speed, playing }
        │
        ▼  RUNTIME (zéro allocation)
Canvas::draw_pixel_matrix_alpha (décor)  /  Canvas::draw_sprite_affine × N (rig animé)
```

- **Format pixel** : RGBA prémultiplié `0xAARRGGBB` (produit natif de tiny-skia) → blend
  `out = src + dst*(255-a)/255` via l'identité `div255` déjà en place.
- **Données lourdes en `Arc`** portées par l'entité : aucun conflit d'emprunt dans `render(&mut self)`,
  partage des bitmaps entre entités, JSON de scène léger (`SvgRef { asset }` uniquement).

## 5. Réalisations

### P1 — Socle vectoriel (décor statique)
- `vector.rs` : `RasterImage`, `rasterize_svg`, `parse_svg`, `rasterize_tree`, `SvgCache` (déduplication),
  plafond dimensionnel 1024².
- `entity.rs` : `ShapeKind::SvgRef { asset }` + composant `VectorSprite`.
- `renderer.rs` : `draw_pixel_matrix_alpha` (chemin opaque rapide `alpha==255` + blend prémultiplié).
- `game.rs` : résolution à l'import dans `load_from_scene` + branche de rendu `SvgRef`.

### P2 — Rig & animation
- `vector.rs` : `RigPart`, `RigAsset`, `Keyframe`, `AnimTrack`, `AnimClip`, `extract_rig`,
  `rasterize_part` (pixmap à la taille du `abs_layer_bounding_box`), `clips_from_json` + `default_clips`.
- `renderer.rs` : `draw_sprite_affine` (rotation/échelle autour d'un pivot, mapping inverse, nearest).
- `game.rs` : avance `time` dans `update`, composition du rig dans `render` (`render_svg_rig`).

### P3 — Pilotage KVL & démonstration
- `interpreter.rs` : 3 actions d'animation (variables `anim_request` / `anim_request_entity`).
- `game.rs` : consommation des requêtes (idempotence, purge de la variable).
- Assets : `decor_torch.svg`, `decor_crate.svg`, `hero.svg` (6 groupes), `hero.anim.json` (idle/walk).
- Scène `assets/scenes/svg_demo_01.scene.json` enregistrée dans `AVAILABLE_SCENES`.

### P4 — Performance & conformité
- Benchs SVG dans `engine_bench.rs`, test d'invariance d'allocation, vérification wasm32.

## 6. Mesures chiffrées (`cargo bench`)

| Métrique | Valeur | Cible |
|---|---|---|
| `rasterize_svg` 128² (import) | **32,1 µs** | < 2 ms ✅ |
| `draw_pixel_matrix_alpha` 64² | **3,3 µs / sprite** | — |
| `draw_sprite_affine` ×6 (rig) | **6,6 µs / frame** | — |
| Frame complète (level_01) | **318 µs** | ≤ 16 666 µs ✅ |
| **Marge 60 FPS** | **98,1 %** | — |
| Taille `libkaractirage_engine.rlib` | **12 Mo → 13 Mo** (+1 Mo) | < 60 Mo ✅ |

## 7. Tests

| Suite | Avant | Après |
|---|---|---|
| engine-core | 144 (+1 ignoré) | **161 (+1 ignoré)** |
| kvl-core | 4 | 4 |
| kvl-runtime | 9 | **11** |

Nouveaux cas : rastérisation (déterminisme, erreur, crop, cache), blit alpha (opaque/semi/hors-bornes),
blit affine (identité ≡ matriciel, rotation), extraction de rig, échantillonnage de clip (interpolation +
bouclage), résolution/rendu d'une scène SVG, application d'une requête d'animation KVL, **invariance
d'allocation du hot path de rendu** (120 frames sans réallocation du framebuffer ni de `render_order`).

## 8. Conformité aux documents de vision

| Exigence | Conformité |
|---|---|
| Low-End First (iGPU/software, 60 FPS) | Rendu CPU pur (tiny-skia), aucun GPU requis |
| Binaire < 60 Mo | +1 Mo de dépendance, 100 % Rust |
| Formats ouverts | SVG (W3C) + clips JSON |
| Licences permissives | Apache-2.0/MIT/BSD-3-Clause |
| Zéro allocation par frame | Bitmaps en cache + blit (test d'invariance) |
| Continuum KVL | `PlayAnimation2D*` / `StopAnimation2D` |
| §4.1 « Générateur d'animations 2D » | Clips JSON + rig squelettique (base) |
| IA assistive | Pipeline déterministe, aucun modèle embarqué |

## 9. Limites assumées

- **Pas de SMIL** : un SVG animé existant doit être converti en rig/clips (choix assumé, SMIL est
  non déterministe et difficile à piloter via KVL).
- **Rig de premier niveau uniquement** : seuls les `<g id>` enfants directs de la racine sont extraits
  (groupes imbriqués non supportés en v1).
- **Pivot au centre** du membre (les pivots d'articulation fins seront spécifiables via le clip JSON).
- **Texte SVG non rendu** (`text` désactivé) : le texte reste géré par `fontdue`.
- **Chevauchement de membres** : compositing sans gestion d'occlusion fine (hors périmètre démo).

## 10. Suite (P5)

Import drag & drop `.svg` dans le Studio, pivots configurables, groupes imbriqués, atlas/export
spritesheet, re-rasterisation par profil de qualité (Compatibility/High).

---

*Validation finale : `cargo test --workspace` (176 ✅), `cargo check --workspace` ✅,
`cargo check --target wasm32-unknown-unknown -p engine-core` ✅, `npm run build` ✅, `oxlint` 0 erreur.*

