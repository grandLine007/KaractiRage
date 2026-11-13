# Rapport de version — v0.3.1 « Consolidation + socle P2 »

| Champ | Valeur |
|---|---|
| Version | `v0.3.1` |
| Phase | fin **P1** (dettes) + début **P2** |
| Date | 11 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Commit de référence | à renseigner au tag |
| Rapport précédent | [`RAPPORT_v0.3.0.md`](RAPPORT_v0.3.0.md) |

> Version menée en **orchestration multi-agents** : trois agents en périmètres disjoints
> (replay, UI sauvegarde, manifeste), intégration et gates par l'agent meneur.

## 1. Périmètre annoncé vs livré

| Élément | Livré | Écart |
|---|---|---|
| **AC2** — replay déterministe, même hash | ✅ | le parcours passe par de vraies interactions `E` ; la marche jusqu'aux portes reste une directive scénarisée |
| Écran de sauvegarde/reprise dans le Studio | ✅ | 3 slots + « Reprendre la dernière », `Échap` rend le focus |
| Pré-vol d'export visible dans l'UI | ✅ | refuse avec la liste exacte, échec fermé si moteur injoignable |
| **P2.1** — manifeste d'assets | ✅ | format + validation + budget ; **pas encore d'import** |
| Branchement des scripts KVL à l'ouverture d'un projet | ✅ | déjà livré en v0.3.0 (rappel) |

## 2. Preuves

```text
cargo test --workspace        → 469 passed, 0 failed   (446 → +19 manifeste, +4 replay)
cargo check --workspace --tests → 0 warning
cd apps/studio && npm run build → OK
npm run lint                  → 14 warnings, 0 erreur
npm run audit:ui -- --strict  → 35 captures, 0 violation, dérive max 0,193 % (< 0,5 %)
npm run export:smoke          → OK (9 entités, 0 erreur, 0 réseau)
```

Digest AC2 du jeu de référence : **`c3276c7a110b67f6`** — identique sur deux sessions,
identique après rejeu, et **différent** si une variable ou une position change.

## 3. Ce qui n'est pas livré

| Élément | Raison | Échéance |
|---|---|---|
| Import d'assets (PNG/OGG/GLB) au manifeste | le manifeste est le **format** ; l'import est la brique suivante | P2.2/P2.3 |
| Runtime WASM embarqué (AC3 complet) | init sans système de fichiers | **P6** (lot dédié) |
| Mesure AC1 (< 2 min) | pas de test chronométré | lot UX |
| `max_mesh_triangles` appliqué | `AssetEntry` n'expose pas le compte de triangles | P4 |
| `max_texture_bytes` par sprite | le plafond ne vise que `AssetKind::Texture` | à trancher en P2.2 |

## 4. Métriques

| Métrique | Valeur | Cible | Écart |
|---|---|---|---|
| Tests Rust | **469** | ≥ 446 | +23 |
| Warnings `cargo check` | 0 | 0 | 0 |
| Warnings lint | 14 | ≤ 14 | 0 |
| Violations axe | 0 | 0 | 0 |
| Dérive baseline | 0,193 % max | < 0,5 % | OK |
| Digest AC2 | stable | stable | OK |

## 5. Décisions prises

| Décision | Choix | Justification |
|---|---|---|
| Replay dans le jeu réel | **session externe** + préservation de `state.replay` à travers menu/niveaux | `*self = fresh` écrasait la session : un enregistrement en jeu était impossible |
| Format de replay | champ optionnel `scenario_actions` | compatible avec les replays existants ; directives rejouées au même index de frame |
| Digest d'état | champs limités (scène, niveau, écran, variables triées, entités arrondies) | sensible à 1 px, insensible aux particules/allocs |
| Lecture du manifeste | `from_json` renvoie `ManifestError` | doit pouvoir signaler `VersionInconnue`, pas seulement une erreur de syntaxe |
| Lecture stricte | chaque entrée **ré-enregistrée** à la lecture | un JSON forgé ne peut pas contourner id/chemin/budget |
| Pré-vol UI | échec **fermé** si le moteur est injoignable | mieux vaut refuser que publier un jeu incomplet |
| Boutons du panneau | styles inline à base de tokens | le reset `button` global non layeré écrase les utilitaires Tailwind (voir dette) |

## 6. Écarts et dérogations

| Écart | Cause | Impact | Rattrapage |
|---|---|---|---|
| Replay : téléportation scénarisée vers les portes | la marche n'est pas simulée dans le replay | preuve plus faible qu'une partie jouée au clavier | lot « replay joué » |
| `from_json` du manifeste ne renvoie pas `serde_json::Error` | `VersionInconnue` non représentable | signature différente de la spec | assumé, documenté |
| Panneau de sauvegarde contourne le reset CSS | dette CSS préexistante | styles inline au lieu d'utilitaires | lot CSS (voir §7) |
| Audit ne capture pas le panneau | le harnais n'ouvre pas ce panneau | couverture visuelle par vérification axe séparée | ajouter un écran d'audit |

## 7. Dette technique connue

| Dette | Sévérité | Coût | Quand |
|---|---|---|---|
| `index.css` : `button { background: transparent; color: inherit }` **écrase les utilitaires Tailwind** de tous les boutons | **élevée** (piège permanent pour tout nouveau composant) | 0,5 j + mise à jour de baseline | v0.4.x |
| Panneau de sauvegarde absent du harnais d'audit | moyenne | 0,5 j | avec le lot CSS |
| `max_mesh_triangles` non appliqué | faible | à P4 | P4 |
| AC1 non chronométré | moyenne | 0,5 j | lot UX |
| 64 commits non revus (`feat/ux-consolidation` vs base) | moyenne | revue humaine | humain |
| Token PAT `admin` + `workflow` | moyenne | réduction de scope | humain |

## 8. Risques

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| Le manifeste reste un format sans import | moyenne | P2 perçue comme abstraite | livrer l'import PNG/OGG en P2.2/P2.3 |
| Deux agents Rust simultanés cassent la CI | faible | build rouge | règle : un seul agent Rust par vague |
| Dérive CSS tant que le reset global existe | élevée | incohérences visuelles | lot CSS planifié |

## 9. Gel et prochaine version

- **Gelé** : format `SaveState`, format de replay (compatible), format `AssetsManifest`
  v1, panneau de sauvegarde.
- **Prochaine version** : `v0.4.0` — **P2.2 import Tiled** puis **P2.3 audio par fichiers**
  (Kira), avec le manifeste comme passage obligé.
- **Décision de séquence recommandée** : Tiled avant l'audio (le niveau importé se voit
  immédiatement) ; le runtime WASM reste un lot dédié avant v1.0.

## 10. Validation

- [x] Gates vertes et citées (§2)
- [x] Écarts consignés (§6)
- [x] Dette inscrite (§7)
- [x] Périmètre gelé annoncé (§9)
- [ ] Tag `v0.3.1` créé par un humain
