# Rapport de version — v0.4.1 « Le .tmx entre dans le Studio »

| Champ | Valeur |
|---|---|
| Version | `v0.4.1` |
| Phase | **P2.2** (suite) |
| Date | 16 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.4.0.md`](RAPPORTS/RAPPORT_v0.4.0.md) |

> L'import Tiled n'est plus un binaire en ligne de commande : on **dépose une carte** sur
> le Studio et le projet s'ouvre.

## 1. Périmètre annoncé vs livré

| Élément | Livré | Écart |
|---|---|---|
| `import_tmx_to_project` (scène + projet `.karacti`) | ✅ | écrit à côté de la carte |
| Commande Tauri `engine_import_tiled` | ✅ | refus non-`.tmx`, erreurs françaises |
| Zone de dépôt dans le Hub | ✅ | 32 px, focusable, `aria-label`, état « bureau uniquement » hors Tauri |
| Filtre `.tmx` + toasts + anti-double-import | ✅ | première carte seulement si plusieurs |
| **Drop natif réel** | ❌ | pas de fenêtre Tauri interactive dans cette session |

## 2. Comportement

| Situation | Résultat |
|---|---|
| Drop d'un `.tmx` | import, projet écrit, ouvert, entrée ajoutée aux récents, toast + avertissements |
| Drop d'un autre fichier | toast d'erreur, **aucun fichier touché** |
| Plusieurs `.tmx` | seule la première est importée, toast d'information |
| Mode navigateur (sans Tauri) | zone en retrait, « application de bureau uniquement », aucune exception |
| Dossier source en lecture seule | erreur française explicite |

Le projet importé contient **une scène**, des métadonnées dérivées du nom de la carte, et
les écrans **HUD + Pause** (pas de Menu, pour que la carte ne soit pas masquée).

## 3. Preuves

```text
cargo test --offline --workspace → 547 passed, 0 failed
cargo check --offline --workspace --tests → 0 warning
npm run build → OK
npm run lint → 14 warnings, 0 erreur
npm run audit:ui -- --strict → 39 captures, 0 violation, 0 dérive (après régénération)

export de référence → 3 niveaux, 24 entités, 5041 Ko, 270,4 FPS
```

Tests ajoutés (moteur) : projet écrit et **relu par `KaractiProject::load_from_file`**,
scène **rechargée par `GameState::load_from_scene`** (4 entités, joueur présent),
destination explicite, aucun fichier créé dans `assets/`.

## 4. Décision de baseline (meneur)

La bande de dépôt déplace le contenu du Hub : 4 captures en dérive (10 à 27 %). J'ai
inspecté la capture (`hub@1440×900`) — la bande est propre, alignée, lisible, et
n'ajoute pas de bruit. **Baseline régénérée** (39 captures), `--strict` repasse à
0 dérive, 0 violation.

## 5. Limites inscrites

1. **Drop natif non testé en conditions réelles** : la commande compile, le chemin moteur
   est testé, le listener se charge sans exception en IPC simulé, la zone est vérifiée en
   Playwright — mais aucune fenêtre Tauri interactive n'était disponible. À valider avec
   `cargo tauri dev`.
2. Le test Playwright du drop était un script inline, non rejouable par une commande du
   dépôt.
3. Le projet est écrit **à côté de la carte** ; réimporter la même carte écrase les deux
   fichiers.
4. Erreur console préexistante en **mode navigateur pur** (`GameViewport.tsx`) : sans
   Tauri, `invoke` n'existe pas. Sans rapport avec l'import.

## 6. Validation

- [x] Gates vertes (§3)
- [x] Chemin moteur testé de bout en bout (écriture, relecture, chargement)
- [x] Baseline régénérée sur décision motivée (§4)
- [x] Limites inscrites (§5)
- [ ] Tag `v0.4.1` créé par un humain
