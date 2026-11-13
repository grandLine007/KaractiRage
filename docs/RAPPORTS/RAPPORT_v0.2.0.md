# Rapport de version — v0.2.0 « Édition IA inline »

| Champ | Valeur |
|---|---|
| Version | `v0.2.0` |
| Phase | socle IA (pré-P1) |
| Date | 10 novembre 2026 |
| Statut | **rétroactif** — version jamais taguée, rapport établi a posteriori |
| Commit de référence | `fcc6932` (docs stratégie) |
| Rapport précédent | — (premier) |

> Ce rapport est rétroactif : la v0.2.0 n'a pas été taguée. Il sert de **ligne de base**
> et d'exemple réel du modèle. Les versions suivantes suivront le processus complet
> (rapport avant tag).

## 1. Périmètre annoncé vs livré

| Élément annoncé | Livré | Écart |
|---|---|---|
| Contrat d'édition (`EditContext`, `TargetRef`, `EditProposal`, transaction) | ✅ | — |
| Ops de mutation | ✅ **13** (au lieu de 11 prévues) | `SetTransform3D` et `SetSceneSetting` ajoutés, `EntityId(usize)` au lieu de `u32` |
| Résolution de cible + preview + apply + undo | ✅ | — |
| Prompt micro alimenté par le schéma généré | ✅ | schéma dérivé de `schemars`, plus de `SCHEMA_HINT` recopié |
| Résolveur déterministe sans LLM | ✅ 18 tests | couvre 10 familles d'instructions |
| Commandes Tauri `ai_context` / `ai_edit_*` | ✅ 6 commandes | — |
| Barre inline ancrée + `✦` par champ | ✅ | `Ctrl+K` au lieu de `I` (conflit avec l'Atelier IA) |
| Branchement de l'appel LLM (chemin N1) | ❌ | l'UI s'arrête au prompt et propose l'Atelier IA |
| Ghost de prévisualisation | ❌ | reporté, gelé avec la barre |
| CI GitHub Actions | ✅ 3 jobs | ajoutée hors plan initial (décision 7 du contrat, encore ouverte) |

## 2. Preuves

```text
cargo test --workspace          → 406 passed, 0 failed (base : 344)
cargo check --workspace         → 0 warning
cd apps/studio && npm run build → OK (tsc + vite)
npm run lint                    → 14 warnings, 0 erreur (baseline : 14)
npm run audit:ui -- --strict    → 35 captures, 0 violation axe, 0 overflow, 0 dérive
GitHub Actions (a78c77b)        → 3/3 jobs verts, 240 s
```

Historique des gates : premier run CI rouge (`alsa-sys` non installé sur Linux), corrigé
par `e028d07` ; deuxième run vert en 325/323/334 s.

## 3. Ce qui n'est pas livré

| Élément | Raison | Échéance |
|---|---|---|
| Chemin LLM inline (prompt → modèle → `ai_edit_propose`) | aucune commande de complétion texte générique ; `ai_step` génère un niveau complet | après v0.3.0 |
| Ghost de prévisualisation | nécessite un canal de prévisualisation vers le renderer | gelé |
| Barre inline KVL, batch, 3D | hors du périmètre du sprint 2, gelé | après v0.3.0 |
| PR vers la branche d'intégration | `feat/ux-consolidation` était **64 commits** d'avance : une PR aurait été un merge de consolidation | pilote après P1 |
| Labels GitHub, protection de branche | actions humaines (contrat §4.3) | humain |

## 4. Métriques

| Métrique | Valeur | Cible | Écart |
|---|---|---|---|
| Tests Rust | 406 | ≥ 344 | +62 |
| Warnings lint | 14 | ≤ 14 | 0 |
| Violations axe (critical/serious) | 0 | 0 | 0 |
| Dérive baseline audit | 0 | 0 | 0 |
| Ops disponibles | 31 | 31 | 0 |
| Warnings `cargo check` | 0 | 0 | 0 |

## 5. Décisions prises

| Décision | Choix | Justification | Document |
|---|---|---|---|
| D1 — identité de contribution | compte personnel + trailers `Agent:`/`Task:` | mise en route immédiate ; machine user écarté pour l'instant | contrat §3.1 |
| Canal PR | template créé, workflow CI ajouté, **aucune PR ouverte** | 64 commits d'écart = consolidation, pas une PR de tâche | contrat §5.5 |
| Édition IA | barre inline, **pas** de modale | plan IA inline §3 | PLAN_IA_INLINE_EDITING |
| Raccourci | `Ctrl+K` | `I` déjà pris par l'Atelier IA | ce rapport |
| Stratégie 2D/3D | Studio = assembleur, pas modeleur 3D | contrainte low-end + souveraineté | STRATEGIE_CREATION_2D_3D_IA |

## 6. Écarts et dérogations

| Écart | Cause | Impact objectif | Rattrapage |
|---|---|---|---|
| `TargetRef::EntityId(usize)` au lieu de `u32` | `Entity::id` est `usize` dans tout le moteur | nul | assumé, documenté |
| `TargetRef` en JSON à représentation externe | serde interdit `tag = "kind"` sur des newtypes scalaires | nul (forme de l'annexe du plan) | assumé |
| `AllOfKind` ≡ `Tag` | pas de champ `kind` distinct dans `Entity` | faible : vocabulaire du prompt seulement | à revoir si un `kind` apparaît |
| Deux piles d'annulation (IA vs moteur) | transactions IA hors de `engine_undo` | modéré : `Ctrl+Z` contextuel | désarmé sur édition manuelle ; unification après P1 |
| Audit visuel exclu de la CI | rendu de polices dépendant de la plateforme | nul (reste une gate locale) | assumé |

## 7. Dette technique connue

| Dette | Sévérité | Coût estimé | Quand |
|---|---|---|---|
| **Les tests écrivent dans le dépôt** (`export/standalone_game.html`, `assets/scripts/guided_*_43/44.kvl.json`) | **élevée** — resalit l'arbre à chaque `cargo test`, contredit la DoD | 0,5 j | ✅ **corrigé** (voir §11) |
| Versions incohérentes : `Cargo.toml` 0.1.0, `package.json` 0.0.0, UI codée en dur « v0.2.0 » | moyenne | 0,5 j | ✅ **corrigé** (voir §11) |
| `App.tsx` : handler clavier avec bloc `F6` dupliqué | faible | 15 min | reporté |
| Webhooks/erreurs : `ai_edit_preview` conflate clarification et erreur | faible | 2 h | v0.4.0 |
| 64 commits non revus entre `feat/ux-consolidation` et `feat/tauri-react-studio` | moyenne | revue humaine | humain |

## 8. Risques ouverts

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| Token PAT avec droits admin + `workflow` | élevée | modification de CI possible par l'agent | réduire le scope une fois la CI stable |
| `feat/ux-consolidation` diverge encore | moyenne | merge douloureux | PR de consolidation quand l'UX sera validée |
| Dérive de périmètre pendant P1 | **élevée** | v0.3.0 jamais livrée | gel écrit dans `P1_JEU_2D_COMPLET.md` |
| MCP expérimental (P3/P4) | moyenne | instabilité des packs | packs isolés, jamais obligatoires |

## 9. Gel et prochaine version

- **Gelé à partir de cette version** : la barre d'édition IA et les 31 ops. Bugs oui,
  nouvelles surfaces non.
- **Prochaine version** : `v0.3.0` — phase **P1**, boucle « jeu 2D complet ».
- **Critères d'acceptation** : AC1-AC6 de [`P1_JEU_2D_COMPLET.md`](../P1_JEU_2D_COMPLET.md).

## 10. Validation

- [x] Gates vertes et citées (§2)
- [x] Écarts consignés (§6)
- [x] Dette inscrite (§7)
- [x] Périmètre gelé annoncé (§9)
- [ ] Rapport rétroactif : pas de tag (assumé)

## 11. Corrections après rédaction (avant v0.3.0)

Deux dettes du §7 ont été soldées immédiatement, avant d'ouvrir P1 :

1. **Écritures de test hors du dépôt.**
   `GameState.export_root` (dossier d'export configurable) et
   `NativeEngine::set_workspace_root` (racine d'authoring) ont été introduits ; les tests
   concernés les pointent vers un répertoire temporaire. Preuve : après
   `cargo test --workspace` (406 tests), `git status` ne montre **aucun** fichier sous
   `assets/` ni `export/`.
   Effet de bord utile : un projet ouvert hors du dépôt n'écrit plus dans le répertoire
   courant du processus (ancien comportement basé sur `CARGO_MANIFEST_DIR`).

2. **Version unique.**
   Source de vérité : `Cargo.toml [workspace.package] = 0.2.0`, héritée par
   `src-tauri` via `version.workspace = true`. `package.json` et `tauri.conf.json`
   alignés ; l'interface lit `__APP_VERSION__` injectée par Vite (plus de « v0.2.0 »
   codé en dur). Garde-fou : `npm run check:versions`, exécuté dans le job CI `studio`.

Restent ouvertes : le bloc `F6` dupliqué, la confusion clarification/erreur de
`ai_edit_preview`, la revue des 64 commits.
