---
name: pxl-pixel-art
description: >-
  Create and edit KaractiRage pixel-art game sprites with pxl (a compact JavaScript DSL that
  renders PNGs and sprite sheets). Use when asked to draw, design or iterate on game sprites,
  tiles, or animated assets — or when a generated level must reference real pixel-art via
  sprite_asset / sprite_frame.
---

# pxl : pixel art as code (KaractiRage)

`pxl` transforme un fichier `.px.js` en PNG, sprite sheets + JSON (TexturePacker/Aseprite),
GIF et un viewer navigateur live qui rejoue le dessin coup par coup.

## Où ça vit

- **DSL / CLI** : `tools/pxl/` (vendu, licence MIT). Le CLI s'exécute via le wrapper
  `node tools/pxl/run.js` (Node ≥ 22.14 ; la build `dist/` est régénérée par le script).
- **Sources d'art** : `assets/art/*.px.js` (la source officielle des sprites de spawn).
- **Sorties** : `assets/generated/` (committées) :
  - `sprites/<name>.png` — sprites statiques
  - `anims/<name>.png|json|gif` — animations par frames
  - `<sheet>.png|json` — atlas (ex: `chars`)

## Commandes

```sh
node tools/pxl/run.js build assets/art/game.px.js --out assets/generated
node tools/pxl/run.js serve assets/art/game.px.js --port 4321 --out assets/generated
node tools/pxl/run.js ascii assets/art/game.px.js <name>
node tools/pxl/run.js list  assets/art/game.px.js
```

Le raccourci reproductible : `bash scripts/build_art.sh`.

## Workflow

1. Écrire / modifier `assets/art/game.px.js` (modèle : `pal`, `ramps`, `sprite`, `sheet`, `anim`).
2. Builder : `node tools/pxl/run.js build assets/art/game.px.js --out assets/generated`.
3. **Regarder** : `assets/generated/_preview/<name>.png` (upscalé, damier = transparence).
   Pour un fix au pixel près : `node tools/pxl/run.js ascii ...` (clés + règles x/y).
4. Itérer par petits patches. Lancer `serve` pour voir peindre en direct.

## Essentiels du DSL

- **Couleurs** : clés 1 caractère via `pal({ k: "#0d0819", ... })`, `"#hex"`, `"."` = skip, `"_"` = efface.
- **Ordre** : les items s'exécutent en séquence (grilles, ops, fonctions, layers), puis
  mirror → swap → flip → rot → outline → crop.
- **Formes** : `rect box line hline vline circ ring ellipse poly fill wire text`.
- **Réutiliser** : `stamp tile pattern tilemap scatter`, `_parts` (non exportés), `group`, globs.
- **Animation** : `anim(name, { fps, frames }, ...)` + `frame(...)` patches, ou procédural via
  `c.phase`. Un `anim` posé dans une scène anime la scène.
- **Recettes qui économisent des tokens** : écrire la moitié d'un sprite symétrique (`mx`),
  `outline` au lieu de contours à la main, recolor via `base` + `swap`.

Référence API complète : `tools/pxl/reference.md`. Workflow agent : `tools/pxl/SKILL.md`.

## Intégration au payload de niveau

Quand un LLM génère un niveau, il peut référencer de la vraie pixel-art au lieu des archétypes
procéduraux via deux champs optionnels par entité :

```json
{ "name": "Piece", "kind": "item", "sprite_asset": "coin" }
{ "name": "Coffre", "kind": "item", "sprite_asset": "chars", "sprite_frame": "chest" }
```

- `sprite_asset` : nom d'un sprite statique (`sprites/<asset>.png`) **ou** d'une spritesheet
  (`<sheet>`), résolu par le moteur à l'import (`ShapeKind::ImageRef` / `SheetRef`).
- `sprite_frame` : frame nommée de l'atlas (à combiner avec `sprite_asset` = nom de la sheet).
- Valeurs connues (assets de spawn) : `coin`, `potion`, `key`, `chest`, `door`, `platform`,
  `spikes`, `crate`, `plate`, `boss`, `wall`. Atlas : `chars` (frames = ces 11 noms).
  Animation : `coin_spin` (via `sprite_asset` sur une entité `AnimRef` si besoin).

Priorité de rendu côté moteur (`ai_patch.rs`) : `sprite_asset` > `sprite_ascii` >
`sprite_archetype` > détection automatique. Un asset manquant retombe sur le rectangle coloré.
