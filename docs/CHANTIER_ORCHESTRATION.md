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

## 3bis. Vague 3 — terminée

| Agent | Tâche | Périmètre | Critère de sortie |
|---|---|---|---|
| δ « manifeste-prévol » | Brancher le manifeste sur le projet et le pré-vol d'export | `project.rs`, `game.rs`, `exporter.rs`, `assets_manifest.rs` | roundtrip projet, pré-vol bloquant sur fichier absent, avertissement `manifeste-absent` |
| ζ « CSS » | Corriger le reset `button` non layeré | `index.css` (+ composant si illisible) | utilitaires Tailwind de nouveau effectifs, build/lint inchangés, rapport de dérive sans mise à jour de baseline |

**Contrainte d'environnement (bloquante pour P2.2)** : `crates.io` répond **403** depuis
cette machine et le crate `tiled` n'est pas en cache local. **Aucune nouvelle dépendance
ne peut être ajoutée** dans cette session. L'import Tiled (P2.2) et l'audio Kira (P2.3)
sont donc reportés à une session avec accès réseau, ou devront être fournis en vendor.

## 3ter. Vague 4 — en cours (débloquer AC3)

| Agent | Tâche | Périmètre | Critère de sortie |
|---|---|---|---|
| η « wasm-init » | Rendre le moteur démarrable en wasm32 sans système de fichiers | `project_registry.rs`, `native_api.rs`, `game.rs`, `plugin/**`, `engine-wasm/src/lib.rs` | build wasm OK + boot Chrome sans panic + framebuffer non vide |
| θ « CSS 2 » | Reste de la cascade + écran d'audit du panneau de sauvegarde | `index.css`, `App.css`, `scripts/audit-ui.mjs` | build/lint inchangés, dérive justifiée, capture du panneau |

**Diagnostic qui a lancé la vague** : `ProjectRegistry::home_dir()` retombe sur
`std::env::temp_dir()`, qui **panique** sur wasm32 (`RuntimeError: unreachable`).
C'était la cause exacte de l'export non jouable (AC3).

**Vague 5 (après η)** : inliner le runtime (glue + wasm) dans le HTML exporté, puis
remettre le test de fumée au vert avec un vrai jeu animé — c'est la promesse
« partager un jeu » qui se ferme.

## 3quater. Vague 5 — AC3 fermé (runtime embarqué)

| Agent | Tâche | Résultat |
|---|---|---|
| λ « runtime-embarqué » | Inliner la glue + le wasm dans le HTML exporté | **export jouable en file://** : 4,7 Mo, `runtime=running`, 960000 px non noirs, 0 erreur, 0 réseau ; deux exports identiques (sha256) |
| meneur | Parité de l'export alternatif + message du script wasm | `to_standalone_html` embarque aussi le runtime |

**Bug découvert au passage** : l'ordre des `HashMap` rendait deux exports du même projet
**différents**. Corrigé par une sérialisation JSON à clés triées — condition nécessaire
pour que l'empreinte et la reproductibilité aient un sens.

**Vague 5b en cours** : entrées clavier dans le template (`wasm_on_key_down/up`), sans
lesquelles le jeu s'affiche mais ne se joue pas.

## 3quinquies. Vague 5b — entrées (AC3 complet)

| Agent | Tâche | Résultat |
|---|---|---|
| μ « entrées wasm » | `wasm_on_key_down/up`, `wasm_on_mouse_move`, `KEY_MAP` dans le template, `--expect-input` | **preuve de jouabilité** : 5778 px modifiés par `ArrowRight`, centroïde du joueur +351 px |
| meneur | relâchement des touches sur `blur`/`visibilitychange` | évite une touche collée après alt-tab |

**AC3 fermé.** État final : export 4,86 Mo, `runtime=running`, 0 réseau, 0 erreur,
déplacement au clavier prouvé, exports identiques au bit près.

## 3sexies. Vague 6 — terminée (P1 soldé)

| Agent | Tâche | Périmètre | Critère de sortie |
|---|---|---|---|
| ν « runtime-périmé » | Rendre visible un `dist/pkg` construit pour une autre version du moteur | `exporter.rs`, `bin/export_game.rs`, `scripts/build_wasm.sh`, template | avertissement `runtime-perime`, méta `karacti-runtime`, tests avec faux runtime |
| ξ « AC1 + Échap » | **Mesurer** « jouable en < 2 min » et empiler les couches pour `Échap` | `App.tsx`, `scripts/first-game.mjs`, `package.json` | ✅ **3,3 s** (`npm run first:game`), pile LIFO testée dans les deux ordres |
| meneur | Bug du hub (hauteurs SVG négatives, 9 erreurs console) + alignement 0.3.4 | `ProjectHubModal.tsx`, versions | ✅ 0 erreur console, `check:versions` vert |

**Pourquoi ces deux-là** : AC1 est le dernier critère de P1 jamais mesuré, et le
garde-fou runtime répond au risque « un export tourne avec un moteur périmé sans le
dire », découvert en vague 5.

**Note CI** : les runners GitHub ont subi une panne DNS (`Temporary failure resolving
azure.archive.ubuntu.com`) sur les runs `116e120`/`aa660aa`/`d3aefcf`. Le job Studio est
passé à chaque fois ; les jobs Rust échouent dans `apt-get`, avant toute compilation.
Correctifs appliqués : timeouts d'étape, retries, et chemin rapide `dpkg`.

## 3septies. Vague 7 — terminée (le boss combat) + vague 8 en cours

| Agent | Tâche | Périmètre | Critère de sortie |
|---|---|---|---|
| « combat de boss » | Le TITAN GARDIEN du niveau 3 doit être vaincu pour finir | `ref_niveau_3.scene.json`, scripts KVL, `game.rs` | test : la porte refuse tant que le boss vit ; tirer jusqu'à sa mort déclenche la victoire |

**Écart visé** : le rapport v0.3.0 assumait « boss gardien, sans combat ». Le moteur
savait déjà tirer, infliger des dégâts et gérer des phases de boss : le manque était
**de contenu**, pas de système.

**Résultat** : porte verrouillée tant que le Titan vit, victoire par le combat seul,
et un **bug moteur** corrigé — les projectiles du boss s'annulaient dans la grille
spatiale, il ne pouvait presque jamais toucher le joueur.

**CI** : verte sur `7c8e938` et `c051ffb` (la panne réseau des runners est passée).

## 3octies. Vague 8 — terminée (l'export contient les 3 niveaux)

| Agent | Tâche | Critère de sortie |
|---|---|---|
| « export multi-niveaux » | Embarquer `game.levels` dans le paquet et reconstruire un `GameState` multi-niveaux à l'import | ✅ **3 scènes, 24 entités, 4 graphes**, `niveaux=3/3` au smoke |
| meneur | Résumé du CLI (comptait l'état courant) + version 0.3.6 | ✅ « 3 niveau(x), 24 entités, 4 graphes KVL » |

**Défaut visé** : l'export ne sérialisait que la scène courante — le fichier partagé
contenait 9 entités (niveau 1) alors que le jeu en compte 3.

## 3nonies. Vague 9 — terminée (n'importe quel jeu s'exporte)

| Agent | Tâche | Critère de sortie |
|---|---|---|
| « réinjection » | Faire survivre graphes KVL et assets embarqués aux changements de niveau | ✅ script absent du disque + PNG + spritesheet survivent à `start_game`/`next_level`/`reload` |
| meneur | Ré-export des images embarquées | ✅ ferme la boucle export → import → export |

**Dette visée** (rapport v0.3.6 §6.1) : `from_scene_data` reconstruit l'état et perd
l'union du paquet. Le jeu de référence n'est pas touché, mais un jeu exporté avec des
sprites importés perd ses images au niveau 2.

## 3decies. Vague 10 — terminée (le jeu exporté a une interface)

| Agent | Tâche | Critère de sortie |
|---|---|---|
| « écrans exportés » | Embarquer `ui_screens` et afficher le niveau courant | à l'import, le menu est **visible** ; « NIVEAU X/N » dans le HUD ; smoke `--expect-screens` |

**Dette visée** (rapport v0.3.7 §5.1) : l'import force `screen = Menu` mais `ui_screens`
est vide — le monde est figé sans surcouche de départ.

## 3undecies. Vague 11 — terminée (les sprites arrivent dans l'export)

| Agent | Tâche | Critère de sortie |
|---|---|---|
| « SVG exportés » | Embarquer les sources SVG et ne plus écraser un sprite déjà résolu | le joueur n'est plus un rectangle dans le HTML exporté ; capture avant/après |

**Défaut visuel constaté** : dans le jeu exporté, le joueur et le décor sont des
rectangles colorés. `from_scene_data` résout `SvgRef` sur disque, échoue en wasm, et met
`vector_sprite = None`. Le paquet n'embarque aucune source SVG.

## 3duodecies. Vague 12 — en cours (finitions export + performance)

| Agent | Tâche | Périmètre | Critère de sortie |
|---|---|---|---|
| agent-5 | Embarquer les `.anim.json` des sprites SVG | `vector.rs`, `exporter.rs`, `game.rs` | un clip personnalisé survit à l'import et aux reconstructions |
| agent-6 | Instrumenter le temps de frame et ajouter `--expect-fps` | `standalone_template.html`, `export-smoke.mjs` | le jeu exporté est mesuré à ≥ 55 FPS, ou le smoke échoue avec la valeur réelle |

**Réseau** : `crates.io` toujours en 403 — Tiled/Kira restent bloqués.

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
| 11/11 | 3 | ✅ δ (manifeste branché) · ✅ ζ (cascade CSS + contrastes) — baseline régénérée par le meneur |
| 11/11 | 3bis | Gates : 480 tests, 0 warning, audit 35 captures 0 violation, dérive 0 |
