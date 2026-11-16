# Rapport de version — v0.4.0 « P2.2 : l'import Tiled »

| Champ | Valeur |
|---|---|
| Version | `v0.4.0` |
| Phase | **P2** (pipeline d'assets) — lot P2.2 |
| Date | 16 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.3.11.md`](RAPPORTS/RAPPORT_v0.3.11.md) |

> Première version de la phase P2 : un fichier `.tmx` devient une scène jouable, et le
> jeu exporté se joue. Le blocage réseau a été contourné par vendoring.

## 1. Périmètre annoncé vs livré

| Élément | Livré | Écart |
|---|---|---|
| Débloquer `tiled` sans réseau | ✅ | vendoring `tiled` 0.16 + `quick-xml` 0.31 |
| `.tmx` orthogonal → `SceneData` | ✅ | objets → entités, propriétés → composants |
| Binaire `tiled_scene` | ✅ | `map.tmx → scene.json`, codes 0/1/2 |
| Carte de démonstration → **jeu jouable exporté** | ✅ | 16 entités, 9 416 px modifiés par une touche |
| Couches de tuiles individuelles (tilesets) | ❌ | v1 : couche aplatie + avertissement |
| Cartes zstd | ❌ | erreur explicite, actionnable |

## 2. Le déblocage

```text
crates.io/api            bloqué
index.crates.io          bloqué
static.crates.io         200   → archives téléchargeables
```

`vendor/tiled-0.16.0` + `vendor/quick-xml-0.31.0`, `[patch.crates-io]`, `exclude` du
workspace (sinon cargo les inclut comme membres et lance leurs doctests amont), et un
`VENDORING.md` qui documente chaque écart : dépendances optionnelles `ggez`/`sfml`/`zstd`
retirées, features stubbées, `#![allow(warnings)]`.

## 3. Mapping Tiled → moteur

| Tiled | Tag | Composants |
|---|---|---|
| `player` | player | `Movement2D`, collider, `Health 3` |
| `wall` / `collision` | wall | collider solide |
| `door` | door | collider + événement `OnDoorInteract` |
| `enemy` | enemy | patrouille, vision |
| `item` / `pickup` | pickup | trigger + `OnPlayerOverlap` |
| `boss` | boss | `Health 20`, `BossComponent` |
| `switch`, `teleport` | switch/teleport | trigger |
| autre | decor | rect sans collider |

Propriétés : `script`, `solid`, `health`/`hp`, `color`, `name`. Coordonnées pixel
(y-down identique), rotation degrés → radians.

## 4. Bug de fond corrigé au passage

`KaractiProject::normalize_levels()` faisait `scenes[0] = scene`, avec `scene` (champ
legacy) pour seule source. Un projet dont `scenes` avait été mis à jour (import Tiled,
édition d'un niveau) mais dont `scene` était périmé **ouvrait la mauvaise scène** — c'est
pourquoi l'export du projet Tiled annonçait 9 entités au lieu de 16.
Désormais `scenes` est la source de vérité et `scene` son miroir.

## 5. Preuves

```text
cargo test --offline --workspace → 545 passed, 0 failed
cargo check --offline --workspace --tests → 0 warning
npm run check:versions → 0.4.0 partout

import de la carte de démo → 16 entités (boss=1, decor=1, door=1, enemy=1, pickup=3,
player=1, switch=1, teleport=1, wall=6), 640×384 px, 1 couche aplatie

export Tiled → 1 niveau, 16 entités, 1 graphe, 5031 Ko
smoke Tiled → runtime=running, niveaux=1/1, écrans=4/4, entrée=9416 px, 184,8 FPS
export référence → 3 niveaux, 24 entités, 5041 Ko, 204,2 FPS
```

Tests de l'import (7) : carte minimale, propriétés, joueur absent, couche de tuiles
aplatie, zstd, scène chargeable, ellipse/point.

## 6. Limites inscrites

1. **Couches de tuiles aplaties** : une entité de décor par couche, pas de tilesets ni de
   tuiles individuelles. Prochain palier : tilesets → `SheetRef`.
2. **zstd non supporté** (vendoring) : erreur explicite, réexport en CSV/zlib requis.
3. Tilesets externes `.tsx` : le parseur exige leur présence même si v1 ne les importe
   pas ; une image manquante ne bloque pas.
4. Cartes infinies : couches de tuiles ignorées (objets importés), avertissement.
5. Pas encore d'entrée UI Studio : l'import passe par le binaire `tiled_scene` ; la
   commande Tauri viendra ensuite.

## 7. Validation

- [x] Gates vertes (§5)
- [x] Blocage réseau contourné et documenté
- [x] Carte Tiled → jeu **jouable** exporté
- [x] Bug `normalize_levels` corrigé
- [x] Limites inscrites (§6)
- [ ] Tag `v0.4.0` créé par un humain
