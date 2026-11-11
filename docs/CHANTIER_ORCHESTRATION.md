# Chantier v0.3.1 → P2 — tableau de bord d'orchestration

> **Rôle** : l'agent meneur (DSH) répartit le travail entre agents, fixe les périmètres de
> fichiers pour qu'ils ne se marchent pas dessus, intègre, vérifie les gates et écrit les
> rapports. Le propriétaire du projet voit l'avancement dans l'interface (liste de tâches,
> agents actifs, CI) et dans ce document.
> **Dernière mise à jour** : 11 novembre 2026.

---

## 1. Règles de répartition (pourquoi c'est fiable)

1. **Un agent = un périmètre de fichiers.** Deux agents ne touchent jamais le même fichier
   dans la même vague.
2. **Les agents Rust et TS ne se croisent pas** : le crate se compile en entier, donc deux
   agents Rust simultanés se bloqueraient sur le verrou Cargo et sur leurs erreurs mutuelles.
3. **Le meneur garde l'intégration** : fichiers partagés (`lib.rs`, `native_api.rs`,
   `App.tsx` hors périmètre d'un agent), gates, documentation, commit.
4. **Une vague = des livrables vérifiables** : build, tests, audit, et un résumé exact des
   commandes exécutées par chaque agent.
5. **Rien n'est « fait » sans preuve** : sortie de commande citée, pas d'affirmation.

## 2. Vague 1 — terminée

| Agent | Tâche | Périmètre | Critère de sortie |
|---|---|---|---|
| α « replay » | Fermer **AC2** : replay hashé du jeu de référence | `crates/engine-core/src/replay.rs` | 2 exécutions → même digest ; replay rejoué → même état ; digest qui change si l'état change |
| β « UI sauvegarde » | Rendre la **sauvegarde/reprise visible** | `SaveSlotsPanel.tsx`, `App.tsx`, `engineBridge.ts` | build OK, lint 14/0, panneau accessible, toasts |

Écarts au plan : aucun à ce stade. Les résultats arrivent dans le fil ; l'intégration
(gates + commit) est faite par le meneur.

## 3. Vague 2 — terminée

| Agent | Tâche | Périmètre | Critère de sortie |
|---|---|---|---|
| γ « manifeste » | **P2.1** : manifeste d'assets (provenance, hash, budget) | `crates/engine-core/src/assets_manifest.rs` (+ 1 ligne `lib.rs` par le meneur) | Schéma versionné, hash stable, budget refusé, tests |
| meneur | **Pré-vol visible** dans l'export du Studio | `App.tsx` (après β), `engineBridge.ts` | Un export bloqué affiche la liste des blocages, pas un échec muet |

## 4. Vague 3 — décisions à prendre par le propriétaire

| Sujet | Options | Recommandation |
|---|---|---|
| Runtime embarqué (AC3) | P6 tôt / plus tard | **tôt** : c'est le seul écart produit de la v0.3.0 |
| Import Tiled (P2.2) | maintenant / après le manifeste | **après** le manifeste, sinon les tuiles arrivent hors pipeline |
| Audio par fichiers (P2.3) | maintenant / après Tiled | après : le son procédural suffit à jouer |

## 5. Ce que « voir la suite » signifie concrètement

| Où | Quoi |
|---|---|
| Interface DSH | liste de tâches ci-dessus, agents actifs, ce document |
| Studio (interface web) | écran de sauvegarde, contrôle audio, barre d'édition IA, pré-vol d'export |
| CI GitHub | 3 jobs (moteur, Studio, Tauri) à chaque push |
| `docs/RAPPORTS/` | un rapport par version, écarts inclus |
| `apps/studio/audit/` | 35 captures, baseline versionnée |

## 6. État du dépôt au moment d'écrire

```text
branche : feat/ux-consolidation (synchronisée avec origin)
version : v0.3.0 prête, non taguée (action humaine)
tests   : 446 (moteur) + 34 (KVL) — tous verts
CI      : 3/3 verts au dernier push
gates   : build OK · lint 14/0 · audit 35 captures 0 violation · export-smoke OK
```

## 7. Journal des vagues

| Date | Vague | Résultat |
|---|---|---|
| 11/11 | 1 | ✅ α (replay, AC2) · ✅ β (UI sauvegarde) — intégrés |
| 11/11 | 2 | ✅ γ (manifeste P2.1) · ✅ meneur (pré-vol UI) — intégrés |
| 11/11 | 2bis | Gates : 469 tests, 0 warning, audit 0 violation, dérive 0,193 % |
