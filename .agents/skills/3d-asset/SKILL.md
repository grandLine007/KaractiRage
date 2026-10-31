---
name: 3d-asset
description: >-
  Reference and place 3D meshes in a KaractiRage level. Use when a generated level must
  contain 3D entities (manifest assets or primitives cube/sphere/plane/capsule) via
  mesh_asset / mesh_primitive / position3d / rotation_euler_deg / scale3d.
---

# Assets 3D (KaractiRage)

Le moteur 3D référence des **identifiants d'assets** (`mesh_asset`) ou des **primitives**
(`mesh_primitive`), jamais des binaires. La *frontière de confiance* est le manifest
intégré (`crates/engine-core/src/mesh_assets.rs`, `MeshManifest::builtin()`).

## Assets disponibles (manifest intégré)

| `mesh_asset` | Description |
|---|---|
| `pillar` | pilier composite (2 blocs) |
| `rock` | rocher (sphère écrasée) |
| `platform` | plateforme (bloc aplati) |

Tout `mesh_asset` **absent du manifest est rejeté** et remplacé par un cube rouge
(fallback). Ne pas inventer d'identifiants.

## Primitives (`mesh_primitive`)

`"cube"`, `"sphere"`, `"plane"`, `"capsule"`. Utiliser `mesh_primitive` pour les formes
simples, `mesh_asset` pour les objets composés.

## Champs du contrat (dans `entities[]`)

```json
{
  "name": "Pilier_Garde",
  "kind": "wall",
  "mesh_asset": "pillar",
  "position3d": [3.0, 0.0, -2.0],
  "rotation_euler_deg": [0.0, 90.0, 0.0],
  "scale3d": [1.0, 1.0, 1.0],
  "color_hex": "#A855F7"
}
```

- `position3d` : `[x, y, z]` (y vers le haut). Si absent, dérivé de `offset_x/offset_y`.
- `rotation_euler_deg` : angles d'Euler en **degrés**, ordre XYZ intrinsèque.
- `scale3d` : `[sx, sy, sz]` (défaut `[1,1,1]`).
- `color_hex` : teinte du matériau solide (réutilisée telle quelle).

La présence de l'un de ces champs (`mesh_asset` / `mesh_primitive` / `position3d` /
`rotation_euler_deg` / `scale3d`) fait de l'entité une entité **3D**
(`Transform3D` + `Renderable3D`) ; sinon elle reste 2D.

## Personnages & animations

- Un **personnage** = entité 3D composée (graphe de scène : entités enfants via
  `parent_id`, chacune avec son `mesh_asset`/`mesh_primitive` + `position3d`).
- Les **animations** restent des données KVL/JSON (durées + transitions), jamais du code
  de shader ; le moteur interpole côté noyau (déterminisme).
