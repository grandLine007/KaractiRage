# Passation — Session IA/RAG & sous-systèmes KaractiRage

> **Date** : 2026-11-01 / 2026-11-02
> **Branche** : `feat/ux-consolidation`
> **Objet** : reprendre le travail « IA 100 % + sous-systèmes moteur » (plan `docs/PLAN_INTENTION_IA_RAG.md`).
> **Dernier commit (HEAD local, non poussé)** : `5fb4cf5 feat(studio): commandes ai_start + ai_history`

---

## 1. Objectif global

Permettre à une IA de construire un jeu complet depuis une intention en langage naturel,
avec **validation déterministe** et **contrôle humain**. Le socle a été posé : dictionnaire
compilé, boucle fermée (intention → plan → ops → exécution), durcissement du catalogue KVL,
et sous-systèmes exposés **comme données** (UI/save/export/3D).

**Ce qui reste** est essentiellement du **rendu/runtime** (voir §7).

---

## 2. Ce qui a été fait (16 commits)

### 2.1 Pipeline IA (cœur)
| Commit | Contenu |
|---|---|
| `112c60f` | Dictionnaire compilé (`dist/ai/current`) + assembleur RAG + catalogue KVL durci |
| `0f159b8` | Migration `KARACTI_AI_SPEC.md` (bandeau « document humain », 60→120 Hz) |
| `3918d30` | `IntentEnvelope` + `AiPlan`/`AiOp` + validation topologique |
| `76f93e6` | Exécuteur de plan : clone + commit atomique + rollback |
| `0ce8d3f` | Ops KVL (`AddKvlNode` + `ConnectPins`) avec validation catalogue |
| `d0f60bc` | Orchestrateur : boucle fermée intention → prompt → LLM → patch |
| `3152b7f` | Ops entité (`GenerateSprite` + `SetEntityComponent`) |
| `7f456ea` | Traçabilité des runs (`.karacti/ai-runs/*.json`) |

### 2.2 Correctifs LLM / Studio
| Commit | Contenu |
|---|---|
| `06314fb` | Endpoint LLM complet côté Studio (corrige « POST / » de LM Studio) |
| `4416085` | Appels LLM **async** (plus de gel), timeout 25→120 s, recherche config robuste |
| `05e072c` | Commande Tauri `ai_plan` (plan déterministe) |
| `ada6522` | Bouton « Voir le plan » dans l'Assistant IA |
| `5fb4cf5` | Commandes Tauri `ai_start` + `ai_history` |

### 2.3 Sous-systèmes moteur (data-driven)
| Commit | Contenu |
|---|---|
| `1b1337b` | Modèles déclaratifs : `UiScreen`, `SaveConfig`, `ExportTargetKind`, `Camera3DData`, `Light3DData` + ops restantes |
| `3998215` | Export data-driven (`to_game_package` + `export_for_target`) |
| `75c80c5` | Test de non-dérive du dictionnaire + `scripts/ci_ai_dictionary.sh` |

---

## 3. Nouveaux modules (engine-core)

| Fichier | Rôle |
|---|---|
| `src/ai_dictionary.rs` | Dictionnaire compilé (build/load, hashes FNV-1a, manifest) |
| `src/ai_prompt.rs` | Assembleur : profils fournisseurs, budgets, retrieval lexical (BM25-lite) |
| `src/ai_intent.rs` | `IntentEnvelope` + `normalize_intent` (classifieur léger) |
| `src/ai_plan.rs` | `AiOp` (18 variantes) + `AiPlan`/`AiPlanStep` + `plan_for` + `validate` |
| `src/ai_executor.rs` | `apply_plan`/`apply_op` (clone + rollback) — 17/18 ops supportées |
| `src/ai_orchestrator.rs` | `generate_plan_from_intent` / `generate_patch_from_intent` / `generate_and_record_patch` |
| `src/ai_history.rs` | `AiRunLog`/`AiRunStep` + `record_run`/`list_runs`/`new_run_id` |
| `src/subsystems.rs` | Modèles UI/save/export/caméra/lumières |
| `src/bin/ai_dictionary.rs` | Binaire de compilation du dictionnaire |
| `scripts/ai_dictionary.sh` / `scripts/ci_ai_dictionary.sh` | Génération / vérification du dictionnaire |

### Fichiers modifiés (Rust)
- `src/ai_client.rs` : purge fantômes, trait `LlmBackend` + `CurlBackend`, `send_prompt_with_system_prompt`, `origin_of`, `load_or_default` (recherche vers la racine), timeout 120 s.
- `src/ai_patch.rs` : **validation catalogue** dans `test_in_sandbox` (action/événement inconnu = erreur), fix ids légacy.
- `crates/kvl-runtime/src/actions.rs` : événement `OnDamage`.
- `crates/kvl-runtime/src/actions_batch.rs` : schémas+handlers `SetObjective` et `BossNovaShoot`.
- `crates/kvl-core/src/*.rs` : dérivation `schemars::JsonSchema` sur le vocabulaire graphe (Graph, Node, NodeKind, Pin, Value, Category, Connection…).
- `src/recipe.rs` : `schemars` sur `GameGenre`/`GameDimension`.
- `src/project.rs` : 6 champs (`ui_screens`, `save_config`, `export_target`, `camera3d`, `lights`, `levels`) + `to_game_package`/`export_for_target`.
- `src/lib.rs` : déclarations des modules.

### Fichiers modifiés (Studio / config)
- `apps/studio/src-tauri/src/lib.rs` : commandes `ai_plan`, `ai_start`, `ai_history` (async).
- `apps/studio/src/components/AiAssistantModal.tsx` : endpoint complet, animation de chargement, bouton « Voir le plan ».
- `.gitignore` : `dist/ai/` versionné (`dist/*` + `!dist/ai/`).
- `KARACTI_AI_SPEC.md` : bandeau + 120 Hz.

---

## 4. Architecture / flux (état actuel)

```
Intention (texte libre)
 ├─ normalize_intent → IntentEnvelope (cible, dimension, genre, clarification)
 │
 ├─ (déterministe) plan_for → AiPlan (validate : acyclique)
 │      └─ apply_plan → clone KaractiProject + AiOp (commit atomique / rollback)
 │
 └─ (LLM) assemble_full → LlmBackend (curl/mock) → AiGeneratedGamePayload
        └─ AiPatchBuilder → AiPatch (sandbox + catalogue) → generate_and_record_patch
             └─ ai_history::record_run → .karacti/ai-runs/<run_id>.json
```

### AiOp (18 variantes) — couverture exécuteur
- **Supportées (17)** : `CreateProject`, `CreateScene`, `AddEntity`, `SetEntityComponent`,
  `AddKvlNode`, `ConnectPins`, `SetVariable`, `SetObjective`, `AddLevel`, `AddMesh`,
  `SetMaterial`, `SetCamera`, `AddLight`, `GenerateSprite`, `SetUiScreen`, `SetSaveConfig`,
  `ExportTarget`.
- **Non supportée (1)** : `SetAudio` (l'audio est procédural via `PlaySound:` KVL).

### Dictionnaire (`dist/ai/current/`)
- `manifest.json` (version `2026.11.1`, `engine_api: 1`, hashes FNV-1a)
- `capabilities.json`, `nodes.json` (catalogue **78 nœuds**)
- `schemas/level.schema.json`, `schemas/graph.schema.json`, `schemas/intent.schema.json`,
  `schemas/plan.schema.json`, `schemas/ops.schema.json`
- `playbooks/2d-platformer.md`, `playbooks/3d-scene.md`
- `examples/2d-platformer-boss.json`, `examples/3d-arena.json`

---

## 5. Faits techniques clés (découvertes pendant la session)

1. **Simulation = 120 Hz** (`native_api.rs:22 SIM_DT = 1.0/120.0`), pas 60 Hz.
2. **`kvl_context` du jeu EST le `ExecutionContext`** (`game.rs:212`) → un handler KVL qui
   `set_var(...)` est lu par le HUD (`objective_hud_text`).
3. **`SetVar` ne gère que des entiers** (`h_set_var` parse `i64`). Pour le texte d'objectif,
   un handler dédié **`SetObjective`** a été ajouté (écrit `objective` en `Value::String`).
4. **Catalogue = 78 nœuds** (75 + `OnDamage` + `SetObjective` + `BossNovaShoot`). Les
   **conditions** (`HasItem:`, `HealthLessThan:`, …) ne sont **pas** cataloguées : repli
   interpréteur. La validation catalogue dans `test_in_sandbox` **exclut** les conditions.
5. **Valeurs fantômes purgées** : `OnEntityDefeated`, `WinGame`, `PlayerDeath`, `jump`,
   `SetObjective` (remplacées par des types réels).
6. **Ids légacy corrigés** : `FlockScatter`→`SwarmScatter:1,4.0`,
   `BossTelegraph`→`TriggerBossTelegraph:RadialNova:1.5`, `NavChasePlayer:130.0`→`NavChasePlayer`,
   `BossNovaShoot:16` (nouveau schéma+handler).
7. **`AiClientConfig::load_or_default()`** cherche `karactirage.ai.json` **du CWD vers la
   racine**. `timeout_seconds` par défaut = **120 s**.
8. **Endpoint LLM** : le frontend doit envoyer le chemin **complet** (`/v1/chat/completions`).
   `test_connection` extrait l'origine via `origin_of`.
9. **Commandes Tauri LLM en `async fn`** (sinon blocage du thread principal → gel fenêtre).
10. **`KaractiProject`** a 6 champs rétro-compatibles (`#[serde(default)]`) :
    `ui_screens`, `save_config`, `export_target`, `camera3d`, `lights`, `levels`.

---

## 6. Commandes de validation

```bash
# Tests moteur (280 tests)
cargo test -p engine-core

# Compilation workspace
cargo check --workspace

# Frontend Studio
cd apps/studio && npm run build && npm run lint

# Dictionnaire : régénération + vérification non-dérive
cargo run -p engine-core --bin ai_dictionary -- --out dist/ai/current
./scripts/ci_ai_dictionary.sh
```

---

## 7. Ce qui RESTE (les 4 gros morceaux)

### 7.1 Rendu UI (HUD/menus depuis `UiScreen`) — **priorité recommandée**
- **Données déjà prêtes** : `subsystems::UiScreen` / `UiElement` (`kind`, `label`, `bind`),
  champ `project.ui_screens`, op `SetUiScreen`.
- **Manquant** : le HUD actuel est **procédural et codé en dur** dans `game.rs`
  (`objective_hud_text`, cœurs `♥/♡`). Il faut :
  1. un rendu data-driven qui lit `ui_screens` et affiche `label` + variable liée (`bind`
     vers `kvl_context.get_var(...)`) ;
  2. une bascule menus (HUD/menu/pause/victoire/game over).
- **Point d'entrée** : `game.rs` (rendu HUD) + `renderer.rs`.

### 7.2 Rendu 3D (consommer `camera3d`/`lights`)
- **Données prêtes** : `Camera3DData`, `Light3DData`, champs `project.camera3d`/`project.lights`,
  ops `SetCamera`/`AddLight`/`AddMesh`/`SetMaterial`.
- **Manquant** : les lire dans le pipeline `RenderPacket` / le renderer 3D (pas de caméra/
  lumières consommées aujourd'hui). `MaterialRef` est encore figé `Solid` côté payload IA.

### 7.3 Save runtime (reprise via `save_config`)
- **Déjà là** : `KaractiProject::save_to_file`/`load_from_file`/`to_json`/`from_json` +
  `NativeEngine::export_project_json`/`load_project_json`.
- **Manquant** : le comportement runtime (`enabled`, `slots`, `auto_save`) et la reprise en
  session. Donnée `save_config` non consommée.

### 7.4 Streaming / `ai_review`
- **`ai_step` (streaming)** : nécessite un client LLM **streamant** (aujourd'hui `curl`
  bloquant derrière `LlmBackend`) + parsing JSON partiel + annulation.
- **`ai_review`** : captures/diff de la prévisualisation (dépend du rendu UI/3D).

---

## 8. Recommandations pour la prochaine session

1. **Repartir du rendu UI** (7.1) : c'est le « vrai manque 2D » (décision D2), et les données
   sont prêtes. Commencer par un HUD data-driven minimal (texte + barre) avant les menus.
2. Valider à chaque étape : `cargo test -p engine-core` + `npm run build`.
3. **Ne pas toucher au flux chaud du renderer** sans snapshot des tests visuels
   (`npm run audit:ui`).
4. Pour le streaming, d'abord définir `LlmBackend::post_json_stream` (trait) + mock, puis un
   parseur JSON partiel, avant d'attaquer l'UI d'annulation.

---

## 9. Pièges / gotchas (IMPORTANT)

1. **Artefacts de test** : `cargo test -p engine-core` **régénère** des fichiers suivis :
   `assets/scenes/level_01.scene.json`, `assets/scripts/*.kvl.json`,
   `crates/engine-core/export/standalone_game.html`, et crée `apps/studio/src-tauri/assets/`
   + `apps/studio/src-tauri/ai_llm_*`. **Toujours les restaurer avant commit** :
   ```bash
   git checkout -- assets/scenes/ assets/scripts/ crates/engine-core/export/standalone_game.html
   rm -rf apps/studio/src-tauri/assets/ apps/studio/src-tauri/ai_llm_*
   git clean -fd assets/scripts/ apps/studio/src-tauri/
   ```
2. **Deux tests offline forcés** (`test_ai_patch_trigger_and_review_lifecycle`,
   `test_ai_patch_rejection_and_instant_rollback`) ont `ai_config.provider = OfflineLocal`
   en tête : ne pas le retirer, sinon le config ambiant (`karactirage.ai.json` = LM Studio)
   les fait échouer.
3. **`karactirage.ai.json`** (racine) : `provider=LmStudio`, endpoint complet, timeout 120.
4. **Non-dérive** : toute modif du code qui change le catalogue/schémas **casse**
   `dictionary_matches_committed_dist` → régénérer `dist/ai/current` puis committer.
5. **Conditions KVL non cataloguées** : ne pas brancher `NodeRegistry::validate_graph`
   « brut » sur des graphes avec conditions (utiliser `unknown_catalog_nodes` qui les exclut).
6. **`SetVar` = entiers uniquement** : pour du texte, utiliser `SetObjective` (ou ajouter un
   handler).
7. **Commandes LLM Tauri = `async fn`** obligatoire (blocage sinon).


