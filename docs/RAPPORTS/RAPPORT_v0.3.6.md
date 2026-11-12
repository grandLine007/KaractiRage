# Rapport de version — v0.3.6 « Le boss combat, l'export contient le jeu »

| Champ | Valeur |
|---|---|
| Version | `v0.3.6` |
| Phase | P1 (dettes de contenu et d'export) |
| Date | 12 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.3.5.md`](RAPPORT_v0.3.5.md) |

> Deux vagues : combat de boss (contenu + un bug moteur) et export multi-niveaux.
> Deux défauts de fond découverts, aucun n'était visible dans les tests existants.

## 1. Périmètre annoncé vs livré

| Élément | Livré | Écart |
|---|---|---|
| Le TITAN GARDIEN est un obstacle réel | ✅ | porte verrouillée tant qu'il vit, victoire par le combat seul |
| Le boss peut terrasser le joueur | ✅ | bug de projectiles corrigé (voir §3) |
| **L'export contient les 3 niveaux** | ✅ | 3 scènes, 24 entités, 4 graphes |
| Pré-vol sur tous les niveaux | ✅ | scripts et assets des scènes 2/3 vérifiés |
| Diagnostic `__karacti_levels` | ✅ | smoke `--expect-levels 3` |
| Réparation des assets après changement de niveau | ❌ | voir §6 limite 1 |

## 2. Preuves

```text
cargo test --workspace        → 506 passed, 0 failed
cargo check --workspace       → 0 warning
npm run lint                  → 14 warnings, 0 erreur
npm run check:versions        → 0.3.6 partout

export_game → export OK — 3 niveau(x), 24 entités, 4 graphes KVL, 4949 Ko
export-smoke --expect-runtime --expect-input --expect-levels 3
→ OK — runtime=running, niveaux=3/3, 960000 px non noirs, entrée=7068 pixels modifiés
deux exports → même sha256 (déterminisme conservé)
```

## 3. Bugs de fond découverts et corrigés

| Bug | Nature | Impact évité |
|---|---|---|
| Les projectiles du boss s'inséraient dans la grille spatiale **comme cibles** | moteur | les 12 balles d'une nova s'annulaient à la création : le boss tirait dans le vide |
| `AudioContext is closed` en wasm après un changement de niveau | moteur (web) | `--expect-runtime` échouait : l'audio recréé fermait le contexte précédent |
| L'export ne sérialisait que la scène courante | produit | le fichier partagé contenait 1 niveau sur 3 |
| Le résumé du CLI comptait l'état courant, pas le paquet | outil | « 9 entités » annoncé pour un jeu qui en compte 24 |

Le troisième est le plus grave : la promesse « partager son jeu » ne portait que sur un tiers du contenu, et le boss n'était même pas dans le fichier.

## 4. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Verrou de la porte | KVL `IsBossDefeated` + `OpenDoor` | réutilise les opérateurs existants, aucune règle codée en dur par niveau |
| Paquet multi-niveaux | `scenes: Vec<SceneData>` avec `serde(default)` | rétrocompatible : un ancien paquet reste importable |
| Import | `from_scene_data(scenes[0])` + `levels` + écran Menu | le joueur retrouve le même parcours que dans le Studio |
| Audio wasm | **singleton** partagé (wasm uniquement) | un contexte fermé alors que ses callbacks tournent produit une pageerror |
| Pré-vol | scanne tous les niveaux | un asset manquant au niveau 3 doit bloquer l'export |

## 5. Métriques

| Métrique | Valeur |
|---|---|
| Tests Rust | **506** |
| Niveaux embarqués | 3 |
| Entités totales du paquet | 24 |
| Graphes KVL | 4 |
| Taille de l'export | 4 949 Ko (dont 36,7 Ko de données, le reste = wasm) |
| Pixels modifiés par une entrée | 7 068 |
| Dérive d'audit | 0 (UI inchangée cette version) |

## 6. Limites inscrites

1. **Réinjection après changement de niveau (wasm)** : `start_game()` et `next_level()`
   reconstruisent l'état via `from_scene_data` sans réinjecter l'union KVL ni les assets
   embarqués. Sans effet sur le jeu de référence (ses 4 scripts ont des replis
   `include_str!` et ses scènes n'ont pas d'assets image), mais un jeu exporté avec des
   sprites importés perdrait ses images au niveau 2. **Correctif à planifier.**
2. **`ui_screens` (menu/pause/victoire) non embarqués** : l'import force l'écran Menu,
   mais aucune surcouche n'est rendue — le monde est figé jusqu'à `Entrée`.
3. Pas d'indicateur « niveau X/3 » dans le HUD.
4. Les budgets de pré-vol restent mesurés sur la scène courante.
5. `audio.rs` : singleton wasm à relire en revue (chemin natif inchangé).

## 7. Validation

- [x] Gates vertes (§2)
- [x] Bugs de fond corrigés et prouvés (§3)
- [x] Limites inscrites (§6)
- [ ] Tag `v0.3.6` créé par un humain
