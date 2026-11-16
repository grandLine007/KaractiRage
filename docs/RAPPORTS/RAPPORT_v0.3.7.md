# Rapport de version — v0.3.7 « N'importe quel jeu s'exporte »

| Champ | Valeur |
|---|---|
| Version | `v0.3.7` |
| Phase | P1 (dettes d'export) |
| Date | 13 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.3.6.md`](RAPPORT_v0.3.6.md) |

> Dette §6.1 du rapport v0.3.6 soldée : les graphes KVL et les assets du paquet
> survivent désormais aux changements de niveau, et un ré-export conserve ses images.

## 1. Périmètre annoncé vs livré

| Élément | Livré | Écart |
|---|---|---|
| Graphes KVL du paquet réinjectés après `start_game`/`next_level`/`reload` | ✅ | — |
| Assets embarqués réinstallés après reconstruction | ✅ | — |
| Variables initiales du paquet préservées | ✅ | sans écraser une valeur posée par la scène |
| Ré-export d'un jeu importé conserve ses images | ✅ | corrigé par le meneur en complément |
| `ui_screens` embarqués dans le paquet | ❌ | limite §6.1 |
| SVG et assets 3D embarqués | ❌ | hors périmètre |

## 2. Comment ça marche

`GameState` mémorise le paquet :

| Champ | Rôle |
|---|---|
| `exported_kvl_graphs` | union KVL du paquet (tous niveaux) |
| `exported_initial_variables` | variables initiales du paquet |
| `embedded_sprites` / `embedded_sheets` | octets bruts, pour réinstallation |

À chaque reconstruction (`start_game`, `next_level`, `reload`), `inherit_exported_package`
puis `restore_exported_package` appliquent, dans cet ordre :

1. **assets** (caches + re-résolution des entités) ;
2. **graphes absents** du nouvel état, avec leurs variables initiales ;
3. **variables du paquet** seulement si encore absentes.

Un graphe résolu sur disque **n'est jamais écrasé** : la version du disque (et ses
variables) gagne. Hors import de paquet, les quatre maps sont vides et rien ne change —
le Studio natif est intact.

## 3. Preuves

```text
cargo test --workspace        → 510 passed, 0 failed
cargo check --workspace       → 0 warning
npm run check:versions        → 0.3.7 partout

export_game → 3 niveau(x), 24 entités, 4 graphes KVL, 4956 Ko
export-smoke --expect-runtime --expect-input --expect-levels 3
→ OK — runtime=running, niveaux=3/3, entrée=7947 pixels modifiés
deux exports → même sha256 (déterminisme)
```

Tests ajoutés : réinjection après `start_game`/`next_level`/`reload` (script absent du
disque + PNG + spritesheet + variable de graphe), non-écrasement d'un graphe disque,
paquet vide inoffensif, scène native sans mémoire de paquet, ré-export des images
embarquées.

## 4. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Priorité des graphes | **disque > paquet** | un projet en cours d'édition ne doit pas être « corrigé » par un ancien export |
| Variables | jamais d'écrasement | une valeur de scène est une décision d'auteur |
| Mémoire du paquet | 4 maps sur `GameState` | transitions rares, coût de clone négligeable |
| Ré-export | images embarquées reprises en priorité basse | ferme la boucle export → import → export |

## 5. Limites inscrites

1. **`ui_screens` non embarqués** : un import dans une instance neuve force l'écran Menu
   mais n'a aucune surcouche à dessiner (monde figé jusqu'à `Entrée`). Prochaine dette.
2. Seuls PNG et spritesheets sont mémorisés ; SVG et assets 3D ne voyagent pas.
3. Un `score = 0` posé par `from_scene_data` garde la priorité sur la valeur du paquet
   (conséquence assumée de la règle « pas d'écrasement »).
4. Le niveau courant n'est pas indiqué dans le HUD.

## 6. Validation

- [x] Gates vertes (§3)
- [x] Dette v0.3.6 §6.1 soldée et prouvée
- [x] Limites restantes inscrites (§5)
- [ ] Tag `v0.3.7` créé par un humain
