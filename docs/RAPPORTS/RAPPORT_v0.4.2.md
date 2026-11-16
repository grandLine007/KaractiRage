# Rapport de version — v0.4.2 « Les tuiles ressemblent à des tuiles »

| Champ | Valeur |
|---|---|
| Version | `v0.4.2` |
| Phase | **P2.2** (fin) |
| Date | 16 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.4.1.md`](RAPPORTS/RAPPORT_v0.4.1.md) |

> Une carte Tiled avec tileset est désormais **rendue** dans le jeu exporté. Et deux bugs
> qui empêchaient toute carte réelle d'être importée ont été trouvés en la testant.

## 1. Ce qui change

| Élément | Avant | Après |
|---|---|---|
| Couche de tuiles | un aplat coloré par couche | **une entité `Tilemap`** : atlas + un index par cellule |
| Rendu | rectangle de décor | blit de la tuile recadrée, une fois par tuile distincte |
| Atlas dans l'export | absent | embarqué dans `sprite_assets`, le jeu autonome rend la carte |
| Pré-vol | — | un tileset manquant est un **blocage** `asset-manquant` |

## 2. Les deux bugs trouvés en testant une vraie carte

### a) Aucune carte de plus d'une rangée ne s'importait

Tiled exporte le CSV **une ligne par rangée**. Le crate `tiled` vendoré refusait ces
sauts de ligne (`invalid digit found in string`), et la correction naïve — retirer les
espaces — collait `4` et `1` en `41`, produisant une carte de 323 valeurs au lieu de 336.

Correctif : normalisation du CSV (séparateurs → virgules, valeurs jointes) **avant** le
parseur. Sans cela, seules les cartes d'une rangée passaient — ce que les fixtures ne
montraient pas.

### b) Tout PNG RGB sans alpha était corrompu

`decode_png` appliquait `normalize_to_color8()` sans expansion alpha : le tampon restait
en 3 octets/pixel, lu par `chunks_exact(4)`. Résultat : un atlas de 64×16 devenait
64×16 avec 768 pixels (au lieu de 1024), couleurs décalées, et **panic** au recadrage
(« index out of bounds ») dans le moteur wasm.

Correctif : `Transformations::ALPHA` (expansion RGBA) + validation de la taille du tampon.
Test de régression sur un PNG RGB 2×1 encodé en dur.

### c) Un panic du parseur ne doit pas tuer le processus

Le crate panique sur une carte incohérente. L'import entier est désormais enveloppé dans
`catch_unwind` et renvoie une `CarteInvalide` actionnable.

## 3. Preuves

```text
cargo test --offline --workspace → 559 passed, 0 failed
cargo check --offline --workspace --tests → 0 warning
npm run check:versions → 0.4.2 partout

carte de démo 24×14 tuiles (384×224 px, 4 tuiles) importée → 3 entités
(door, player, tilemap), 0 couche aplatie, 0 avertissement
export → 1 niveau, 3 entités, 0 graphe, 5033 Ko
smoke → runtime=running, niveaux=1/1, écrans=4/4, entrée=2580 px, 274,4 FPS
référence → 3 niveaux, 24 entités, 5063 Ko, 268,9 FPS
```

Capture : `/tmp/carte_demo/tilemap.png` — herbe, eau, bordure de pierre, sable, joueur,
porte. Le décor n'est plus un rectangle.

## 4. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Représentation | une entité `Tilemap` par couche | 336 cellules = 336 blits/frame, pas 336 entités |
| Cache | recadrage une fois par tuile distincte | 4 tuiles distinctes → 4 bitmaps, aucun I/O au rendu |
| Asset | chemin absolu copié à côté de la carte | résolvable quel que soit le cwd ; l'export l'embarque |
| Erreur de parse | panic converti en erreur | une entrée utilisateur ne doit jamais tuer le processus |

## 5. Limites inscrites

1. **Portabilité native** : `asset` est un chemin absolu ; déplacer la carte sans son
   dossier `tilesets/` casse le rendu natif (l'export reste autonome).
2. **Un seul tileset** importé ; les tuiles des autres sont ignorées avec avertissement.
3. **Retournements/rotations** non appliqués ; **animations de tuiles** figées sur la
   première image.
4. Pas d'occlusion culling entre couches : un blit par cellule visible.
5. La mesure FPS peut être instable au démarrage (le premier passage décode l'atlas) ;
   après ~2 s elle se stabilise (274 FPS mesurés).

## 6. Validation

- [x] Gates vertes (§3)
- [x] Deux bugs bloquants corrigés, chacun avec un test de régression
- [x] Carte Tiled rendue dans le jeu exporté (capture)
- [x] Limites inscrites (§5)
- [ ] Tag `v0.4.2` créé par un humain
