# Rapport de version — v0.3.0 « Boucle jeu 2D complète »

| Champ | Valeur |
|---|---|
| Version | `v0.3.0` |
| Phase | **P1** |
| Date | 11 novembre 2026 |
| Statut | **prêt pour revue** — non taguée (le tag est une action humaine) |
| Commit de référence | `a5d3159` |
| Rapport précédent | [`RAPPORT_v0.2.0.md`](RAPPORT_v0.2.0.md) |

> Ce rapport couvre 9 commits (`f910971` → `a5d3159`).

## 1. Périmètre annoncé vs livré

| Élément annoncé (P1) | Livré | Écart |
|---|---|---|
| P1.1 Écrans + navigation (menu/pause/défaite/victoire) | ✅ | — |
| P1.1 Focus clavier « aucune touche avalée » | ⚠️ | non validé sur le jeu de référence |
| P1.2 Progression + sauvegarde (niveau, variables) | ✅ | moteur + commandes ; **pas d'écran de slots** |
| P1.2 Réglages volume/mute persistants, hors partie | ✅ | — |
| P1.3 Jeu de référence 3 scènes, objectif par scène | ✅ | — |
| P1.3 Boss final | ⚠️ | **gardien sans combat** : le niveau se conclut par la porte |
| P1.3 Victoire/défaite reproductibles | ✅ | victoire par **interaction réelle** (touche E) |
| P1.4 Audio branché + contrôle dans l'UI | ✅ | effets procéduraux ; fichiers audio = P2 |
| P1.5 Export + pré-vol | ✅ | pré-vol bloquant, empreinte stable, smoke test |
| P1.5 Export **jouable** | ❌ | runtime WASM sans init sans-disque → **P6** |
| P1.6 Preuves, guide, rapport | ✅ | ce document + guide 10 min |

### Critères d'acceptation

| # | Critère | État |
|---|---|---|
| AC1 | Nouveau jeu jouable en < 2 min | ⚠️ **non chronométré** : le guide fait ~8 min de lecture guidée, aucune mesure formelle |
| AC2 | Replay déterministe menu → victoire, même hash | ⚠️ **partiel** : le test de bout en bout est déterministe, mais ne passe pas par `ReplayManager` ni par un hash de replay |
| AC3 | HTML unique, 0 réseau, 0 erreur console | ⚠️ **partiel** : 0 réseau et 0 erreur console **prouvés**, mais le fichier n'est **pas jouable** |
| AC4 | Progression restaurée après fermeture | ✅ niveau + score restaurés (rechargement complet simulé, pas un vrai redémarrage) |
| AC5 | Aucune régression (tests, audit, lint) | ✅ 446 tests, 0 violation axe, 14 warnings lint |
| AC6 | Jeu construit uniquement avec les outils | ✅ scènes/graphes/écrans déclaratifs, aucun contenu en code de moteur |

## 2. Preuves

```text
cargo test --workspace          → 446 passed, 0 failed
cargo check --workspace         → 0 warning
cd apps/studio && npm run build → OK (tsc + vite)
npm run lint                    → 14 warnings, 0 erreur (baseline : 14)
npm run audit:ui -- --strict    → 35 captures, 0 violation axe, 0 overflow, 0 dérive
npm run export:smoke -- /tmp/jeu.html → OK — 9 entités, 0 erreur, 0 requête réseau
cargo run -p engine-core --bin export_game -- … → export OK, empreinte 7009bf915e8a92a3
GitHub Actions (a5d3159)        → 3/3 jobs verts, 330 s
```

Captures : `apps/studio/audit/baseline/` (35) et `docs/assets/guide/` (5).

## 3. Ce qui n'est pas livré (et pourquoi)

| Élément | Raison | Échéance |
|---|---|---|
| Runtime jouable embarqué dans l'HTML | `NativeEngine::init` panique sur `std::env::temp_dir` en wasm32 : pas d'init sans système de fichiers | **P6**, premier chantier |
| Combat / boss vaincu | demande projectiles, i-frames, feedback, équilibrage | après P1 |
| Écran de slots (sauvegarder/reprendre) | l'UI n'a pas de menu ; les commandes Tauri existent | P6 ou lot UX |
| Musique et sons depuis des fichiers | nécessite Kira + assets | **P2** |
| Import Tiled, manifeste d'assets | hors périmètre P1 | **P2** |
| 2.5D / 3D / glTF / Blender | palier suivant | **P4** |

## 4. Métriques

| Métrique | Valeur | Cible | Écart |
|---|---|---|---|
| Tests Rust | **446** | ≥ 406 | +40 |
| Warnings lint | 14 | ≤ 14 | 0 |
| Violations axe (critical/serious) | 0 | 0 | 0 |
| Dérive baseline audit | 0 | 0 | 0 |
| Taille d'export (HTML) | 20 Ko | < 2 Mo | très en dessous |
| Entités / graphes exportés | 9 / 2 | — | — |
| Empreinte d'export | stable (`7009bf91…`) | stable | OK |
| Durée CI | 330 s | < 600 s | OK |

## 5. Décisions prises

| Décision | Choix | Justification | Document |
|---|---|---|---|
| Format multi-niveaux | **`scenes: Vec<SceneData>` embarquées** (option A) | un `.karacti` exporté doit rester auto-porteur | `P1_JEU_2D_COMPLET.md` §P1.3 |
| Persistance de progression | **enveloppe `SaveState`** (projet + niveau + variables), anciens slots lisibles | sauver le projet seul repartait au niveau 1 | ce rapport §6 |
| Réglages audio | fichier utilisateur séparé (`settings.json`) | couper le son ne dépend pas d'un slot | P1.2 |
| Message d'export | « Export HTML5 (données) — runtime requis » | ne plus afficher « Prêt à jouer » sur un fichier non jouable | P1.5 |
| Scripts KVL | chargés à l'ouverture d'un projet | sinon l'export perdait toute la logique | P1.5 |

## 6. Écarts et dérogations (registre de flexibilité)

| Écart | Cause | Impact sur l'objectif final | Rattrapage |
|---|---|---|---|
| **AC3 partiel** : export auto-porteur mais non jouable | runtime WASM sans init sans-disque | l'utilisateur ne peut pas partager un jeu double-cliquable | **P6** : init wasm sans FS |
| **AC2 partiel** : pas de replay hashé | priorité donnée au parcours réel (interaction de porte) | la déterminisme n'est pas prouvé au sens « replay » | lot « replay + hash » |
| **AC1 non mesuré** | pas de test chronométré | promesse « 2 minutes » non vérifiée | test d'ergonomie guidé |
| **Pas de combat** | système entier à concevoir | le boss n'est pas « vaincu » | après P1 |
| « Quitter/relancer » simulé | pas de redémarrage réel dans les tests | preuve plus faible qu'annoncé | test d'intégration app |
| `project_id` dérivait du nom de scène | dette héritée | sauvegardes éclatées par niveau | **corrigé** en P1.2 |

## 7. Dette technique connue

| Dette | Sévérité | Coût estimé | Quand |
|---|---|---|---|
| Init WASM sans système de fichiers | **élevée** (bloque AC3) | 1-2 j | P6 |
| Pas de replay hashé pour AC2 | moyenne | 0,5 j | v0.3.1 |
| Pas d'écran de slots | moyenne | 1 j | après P2 |
| `cursor_world` absent (« déplace ici » → clarification) | faible | 0,5 j | lot UX |
| Écran de réglages audio ailleurs que le pied de page | faible | — | — |
| 64 commits non revus entre `feat/ux-consolidation` et `feat/tauri-react-studio` | moyenne | revue humaine | humain |
| Token PAT avec droits `admin` + `workflow` | moyenne | réduction de scope | humain |
| `App.tsx` : bloc `F6` dupliqué | faible | 15 min | v0.3.1 |

## 8. Risques ouverts

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| Le WASM révèle d'autres appels disque (`home_dir`, caches) | élevée | P6 plus long | inventaire `cfg(target_arch)` avant de coder |
| Dérive de périmètre pendant P2 | élevée | v0.4.0 glisse | périmètre P2 écrit avant code |
| Le « boss sans combat » déçoit | moyenne | perception | assumé et documenté, combat planifié |
| Régression du contenu de référence | faible | tests E2E | 3 tests de bout en bout |

## 9. Gel et prochaine version

- **Gelé à partir de cette version** : le jeu de référence (3 niveaux, gardien), la
  machine à états d'écrans, le format `scenes` et `SaveState`. On corrige, on n'étend pas.
- **Prochaine version** : `v0.4.0` — phase **P2** (manifeste d'assets, import Tiled,
  audio par fichiers, rechargement à chaud).
- **Critères d'acceptation de v0.4.0** : cf. [`PLAN_INTEGRATION_OUTILS.md`](../PLAN_INTEGRATION_OUTILS.md) §P2.

## 10. Validation

- [x] Toutes les gates vertes et citées au §2
- [x] Aucun écart non consigné au §6
- [x] La dette ajoutée est inscrite au §7
- [x] Le périmètre gelé est annoncé au §9
- [ ] Tag `v0.3.0` créé par un humain — **non fait**
