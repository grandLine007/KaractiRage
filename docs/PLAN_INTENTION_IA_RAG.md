# Plan — Intention IA & dictionnaire RAG pour KaractiRage (2D + 3D)

> **Version** : 1.0 — 1er novembre 2026
> **Objectif** : permettre à une IA de construire un **jeu complet jouable** (2D et 3D)
> depuis une intention en langage naturel, avec validation déterministe et contrôle humain.
> **Périmètre supposé** : le **jeu/projet** produit par le moteur, pas le code source du
> moteur lui-même. Si l'objectif inclut la génération du moteur, voir §14 (décision D0).
> **Statut** : plan directeur — à valider avant implémentation.
>
> **Mise à jour — 3 novembre 2026** : le socle de ce plan est désormais implémenté
> (dictionnaire compilé, intention, plan, exécuteur, prompt budgété, streaming,
> historique, revue ; **344 tests**). La prochaine étape — l'édition à la volée sans
> page dédiée — est traitée dans [PLAN_IA_INLINE_EDITING.md](PLAN_IA_INLINE_EDITING.md).

---

## 0. Thèse

> **Un gros prompt ne construit pas un jeu. Un contrat compilé + une boucle fermée, oui.**

Le projet possède déjà les bons ingrédients : contrats `schemars`, 7 fournisseurs LLM,
génération asynchrone, sandbox, rollback, catalogue de nœuds KVL, modèle 3D et
`RenderPacket`. Ce qui manque n'est pas « plus de texte dans le prompt », c'est :

1. **un dictionnaire généré depuis le code** (pas un document manuel qui dérive) ;
2. **une intention structurée** (cible, dimension, contraintes, critères d'acceptation) ;
3. **un plan multi-étapes** et des **opérations validées** (pas un unique JSON géant) ;
4. **une boucle de validation/réparation** (schéma → sémantique → moteur → visuel → playtest) ;
5. **la couverture des artefacts d'un vrai jeu** (projet, scènes, UI, audio, sauvegarde,
   export, 3D), pas seulement un niveau et quelques entités.

---

## 1. Diagnostic de l'existant

| Élément | État | Limite pour « 100 % IA » |
|---|---|---|
| `KARACTI_AI_SPEC.md` (21 Ko) | Embarqué + relu à chaud, envoyé aux clouds | Monolithe 2D ; 3D absente ; dérive avec le code |
| Prompt compact local | Codé en dur dans `ai_client.rs` | Contient des valeurs **inexistantes** : `OnEntityDefeated`, `WinGame`, `PlayerDeath`, `movement: jump` |
| Catalogue KVL | 75 `NodeSchema` + handlers | Le prompt manuel ne le connaît pas ; `SetObjective` est généré mais sans handler |
| `AiGeneratedGamePayload` | Contrat `schemars` + miroir Pydantic | Limité à **un niveau** ; pas de projet, UI, audio, sauvegarde, export |
| `AiPatchBuilder` | Entités + graphe KVL, sandbox, rollback | N'utilise pas `NodeRegistry::validate_graph` ; actions inconnues acceptées en no-op |
| 3D | `Transform3D`, `Renderable3D`, `MeshRef`, `Camera3D`, `RenderPacket` | Champs présents dans le payload, mais **absents du dictionnaire**, sans caméra/lumières/matériaux/playbook |
| Fournisseurs | 7 (`Ollama`, LM Studio, Claude, Gemini, DeepSeek, OpenAI, Offline) | Pas de profils de contexte, pas de JSON schema contraint, `curl` bloquant, pas de streaming |
| Sidecar `agents/` | CrewAI + contrats Pydantic partagés | Risque de **deux prompts / deux contrats** ; pas de dictionnaire commun |
| Évaluation | Tests unitaires + sandbox | Pas de corpus d'intentions, pas de métrique de complétion de jeu |

### Dérives concrètes à corriger

- `KARACTI_AI_SPEC.md` annonce 60 Hz alors que la simulation est à **120 Hz fixe**.
- Le prompt compact propose `OnEntityDefeated`, `WinGame`, `PlayerDeath`, `jump` :
  ces valeurs ne sont ni dans le catalogue de nœuds ni dans le mapping de `AiPatchBuilder`.
- `SetObjective:<texte>` est créé par `AiPatchBuilder` mais **aucun handler** ne l'exécute ;
  l'objectif affiché vient de `scene_objective`, pas de cette action.
- Les actions générées ne sont pas validées contre `NodeSchema` : une action inconnue
  devient silencieusement un no-op.
- La 3D est documentée dans les rapports, pas dans le dictionnaire envoyé au LLM.

---

## 2. Définition de « 100 % IA »

### 2.1 Échelle de capacité

| Niveau | Définition | Exemple |
|---|---|---|
| **L0** | Répond / conseille | « Comment faire un boss ? » |
| **L1** | Produit un artefact isolé valide | un sprite, un graphe KVL, une entité |
| **L2** | Produit un **niveau** complet et jouable | terrain + entités + logique + objectif |
| **L3** | Produit un **jeu** complet | plusieurs niveaux, menus, UI, audio, sauvegarde, export |
| **L4** | Itère seul jusqu'à acceptance | joue, détecte, répare, rejoue |

**Cible v1** : **L3 en 2D**, **L2/L3 en 3D** selon la décision D1 (2.5D vs 3D complète).
**L4** seulement après un harnais de playtest et des métriques fiables.

### 2.2 Matrice de capacités (contrat de complétude)

| Artefact | 2D | 3D | Aujourd'hui | Cible |
|---|---|---|---|---|
| Projet (`KaractiProject`) | ✅ | ✅ | ⚠️ création locale | L3 |
| Scène / terrain / hazards | ✅ | ✅ | ✅ partiel | L3 |
| Entités / composants ECS | ✅ | ✅ | ✅ partiel | L3 |
| KVL (events/conditions/actions) | ✅ | ✅ | ✅ mais non validé catalogue | L3 |
| Sprites (archétype/ASCII/pxl) | ✅ | — | ✅ partiel | L3 |
| Assets vectoriels / animations | ✅ | — | ⚠️ non exposé IA | L3 |
| UI / HUD / menus | ✅ | ✅ | ❌ non généré | L3 |
| Audio / musique / SFX | ✅ | ✅ | ❌ non généré | L3 |
| Quêtes / dialogues / inventaire | ✅ | ✅ | ⚠️ via KVL | L3 |
| Sauvegarde / progression | ✅ | ✅ | ❌ non généré | L3 |
| Export HTML/WASM/desktop | ✅ | ✅ | ⚠️ outil, pas IA | L3 |
| Meshes / matériaux / caméra / lumières | — | ✅ | ⚠️ champs partiels | L2 |
| 2.5D (rendu 3D + logique 2D) | ✅ | ✅ | ⚠️ prototype | L2 |

> **Complétude = 100 % des lignes de cette matrice pour le niveau visé**, pas 100 % de
> tous les champs possibles du moteur.

**Point de départ mesuré** : L1/L2 partiel en 2D (un niveau, entités, KVL), **0 % de L3**
(pas de projet/multi-niveaux/UI/audio/sauvegarde/export générés), **0 % de L2 3D**
(les champs existent, le dictionnaire et les playbooks non). C'est la ligne de base à
améliorer, pas une critique du moteur.

---

## 3. Architecture cible

```text
                     ┌──────────────────────────────┐
                     │  DICTIONNAIRE COMPILÉ        │
                     │  (Rust → dist/ai/<version>)  │
                     │  schemas + nodes + capas +   │
                     │  playbooks + exemples        │
                     └──────────────┬───────────────┘
                                    │ retrieval + packs
Intention ──► Normalisation ──► Assembleur de prompt ──► Planificateur
   │              │                     │                     │
   │              ▼                     ▼                     ▼
   │        IntentEnvelope        Contexte projet        AiPlan (étapes)
   │                                                        │
   └────────────────────────────────────────────────────────┘
                                                            ▼
                    Générateur d'étape (1 artefact) ◄── Réparation
                                    │
                                    ▼
              Validation : schéma → sémantique → catalogue → sandbox
                           → playtest → visuel → assets → sécurité
                                    │
                                    ▼
                     Ops atomiques appliquées sur un clone
                                    │
                                    ▼
                       Observation (capture/playtest) ──► étape suivante
```

### 3.1 Rust autorité, sidecar orchestrateur

| Responsabilité | Rust (`engine-core`) | Sidecar `agents/` (CrewAI) |
|---|---|---|
| Contrats & schémas | **Autorité** (`schemars`, `NodeSchema`) | Consomme `dist/ai` |
| Planification | Plan officiel, ops, validation | Peut proposer un plan multi-agents |
| Application | **Seul à écrire** dans le projet | Ne touche jamais aux fichiers |
| Validation | Schéma, sandbox, playtest, visuel | Rapporte des critiques, pas des patches |
| Prompt | Assembleur officiel | Même dictionnaire, pas de prompt parallèle |

Le sidecar reste un **orchestrateur d'idées** : il produit un `AiPlan` candidat et des
artefacts, jamais un patch appliqué directement. Un seul dictionnaire, deux consommateurs.

**Deux boucles** :
- **boucle courte** : une étape → un artefact → validation → réparation ;
- **boucle longue** : plan → étapes → jeu → playtest → replanification.

---

## 4. Le dictionnaire : un artefact compilé, pas un document

### 4.1 Couches

| Couche | Contenu | Taille cible | Source | Généré ? |
|---|---|---|---|---|
| **L0 Constitution** | invariants, autorité, sécurité, déterminisme, sortie JSON | ~2 Ko | `ai/constitution.md` | Non (revue) |
| **L1 Capacités** | artefacts, dimensions, statut, limites, schémas | ~3 Ko | `ai/capabilities.yaml` + code | Oui (validé) |
| **L2 Schémas** | JSON Schema de chaque artefact/op | ~15-40 Ko | `schemars` | **Oui** |
| **L3 Catalogue KVL** | 75 nœuds, alias, préfixes, params, handlers | ~20 Ko | `PluginHost::catalog_json()` | **Oui** |
| **L4 Playbooks** | plateforme, RPG, shmup, puzzle, boss, 3D, 2.5D | 2-6 Ko chacun | `ai/playbooks/*.md` | Non (curé) |
| **L5 Exemples** | intention → plan → ops → résultat | 1-3 Ko chacun | `ai/examples/*.json` | Non (golden) |
| **L6 Glossaire** | termes FR/EN, synonymes, pièges | ~2 Ko | `ai/glossary.md` | Non |

### 4.2 Manifeste

```json
{
  "dictionary_version": "2026.11.1",
  "engine_api": 1,
  "schema_hash": "sha256:…",
  "node_catalog_hash": "sha256:…",
  "source_commit": "<git-sha>",
  "dimensions": ["2d", "2.5d", "3d"],
  "artifacts": ["project", "scene", "level", "mechanic", "sprite", "mesh", "audio", "ui"],
  "files": {
    "schemas/level.schema.json": "sha256:…",
    "nodes.json": "sha256:…",
    "playbooks/platformer.md": "sha256:…"
  }
}
```

### 4.3 Compilation

```bash
cargo run -p engine-core --bin ai_dictionary -- --out dist/ai/current
```

Le binaire :
1. sérialise les schémas `schemars` des types Rust ;
2. extrait `NodeSchema` + handlers depuis `PluginHost::with_builtins()` ;
3. valide `capabilities.yaml` contre les schémas ;
4. copie les playbooks/exemples, calcule les hashes, écrit le manifeste ;
5. échoue si un schéma ou un nœud du dictionnaire n'existe pas dans le code.

**Tests de non-dérive** :
- régénérer et vérifier `git diff --exit-code dist/ai` ;
- `nodes.json` contient exactement les 75 nœuds du registre ;
- chaque action/condition/événement des exemples existe dans `NodeSchema` ;
- chaque schéma d'op est un sous-ensemble du schéma de patch ;
- `KARACTI_AI_SPEC` n'est plus la source : il devient un document de vulgarisation
  généré/relu, pas le prompt.

### 4.4 Chargement au runtime

- `AiDictionary::load(path)` vérifie le manifeste, les hashes et `engine_api`.
- Incompatibilité → **refus de générer** (pas de patch approximatif).
- Fallback embarqué via `include_str!` pour le mode hors-ligne.
- Le dictionnaire est **immuable pendant une session** ; version enregistrée dans chaque run.

### 4.5 RAG : récupération typée, pas un vecteur de docs

- Chunks typés : `schema`, `node`, `playbook`, `example`, `glossary`, `capability`.
- Métadonnées : dimension, genre, artefact, moteur, version, tags.
- Recherche **hybride** : BM25/lexical d'abord (déterministe, zéro dépendance), embeddings
  ensuite (Ollama `/api/embeddings` ou modèle local) derrière un flag.
- Top-k + budget tokens ; **citation obligatoire** de la source dans le prompt.
- Les documents projet (README, rapports) sont **non fiables** : jamais d'instructions
  exécutées, uniquement des faits avec provenance.
- Le dictionnaire statique est complété par un **contexte dynamique** :
  projet courant, scène, entités (id/tag), assets disponibles, dimension, erreurs récentes.
  Ce contexte est sérialisé compactement, jamais la scène entière.

### 4.6 Budget de prompt (exemple)

| Bloc | Local 4-8 k | Cloud 32 k+ |
|---|---|---|
| Constitution | 300 | 300 |
| Schéma de l'étape | 800 | 2 000 |
| Nœuds KVL utiles (sous-ensemble) | 500 | 4 000 |
| Playbook de la tâche | 900 | 3 000 |
| 1-2 exemples dorés | 800 | 3 000 |
| Contexte projet compact | 800 | 4 000 |
| Marge / sortie | 2 000 | 12 000 |

Le budget est **calculé avant l'appel** ; si le pack dépasse, on retire d'abord les exemples,
puis le playbook détaillé, jamais la constitution ni le schéma de l'étape.

### 4.7 Exemple de prompt assemblé (local)

```text
[CONSTITUTION]  Règles immuables, sortie JSON, pas de code.
[CAPABILITY]    target=level, dimension=2d, artifacts=[scene, entities, kvl]
[SCHEMA]        level.schema.json (extrait)
[NODES]         OnPlayerOverlap, AddScore, GiveItem, DamagePlayer, SetVar, ...
[PLAYBOOK]      2d-platformer : boucle saut → collecte → porte → boss
[EXAMPLES]      1 exemple « plateforme + clé + boss » (tronqué)
[CONTEXT]       projet=…, scène=…, entités=[…], assets=[…], dernière erreur=…
[INTENT]        « Crée un niveau avec 3 pièces, une clé et un boss »
[OUTPUT]        JSON brut conforme au schéma de l'étape, sans markdown.
```

Le prompt final est **calculé et hashé** ; il ne contient jamais le dictionnaire entier.

---

## 5. Intention, plan, opérations

### 5.1 `IntentEnvelope`

```json
{
  "intent_id": "int_01J...",
  "raw_text": "Crée un jeu de plateforme 2D avec un boss final et une clé",
  "locale": "fr",
  "target": "full_game",
  "dimension": "2d",
  "genre": "platformer",
  "constraints": {
    "levels": 3,
    "difficulty": "accessible",
    "max_entities": 120,
    "art_direction": "dark_atelier",
    "must_include": ["boss", "cle", "checkpoint"]
  },
  "acceptance": [
    "3 niveaux jouables",
    "condition de victoire reachable",
    "aucun KVL invalide",
    "export HTML fonctionnel"
  ],
  "budget": { "max_steps": 12, "max_tokens": 24000 }
}
```

- **Normalisation** : règles + classifieur léger ; si `dimension` ou `genre` est ambigu,
  **poser une question** au lieu de deviner.
- `target` ∈ `project | scene | level | mechanic | asset | full_game | tweak`.
- `acceptance` est la source de la porte de sortie.

### 5.2 `AiPlan`

```json
{
  "plan_id": "plan_01J...",
  "intent_id": "int_01J...",
  "steps": [
    { "id": "s1", "op": "create_project", "artifact": "project", "depends_on": [] },
    { "id": "s2", "op": "create_scene", "artifact": "scene", "depends_on": ["s1"] },
    { "id": "s3", "op": "add_entities", "artifact": "entities", "depends_on": ["s2"] },
    { "id": "s4", "op": "add_kvl", "artifact": "graph", "depends_on": ["s3"] },
    { "id": "s5", "op": "validate_playtest", "artifact": "report", "depends_on": ["s4"] }
  ]
}
```

- Plan **petit** (schéma dédié), généré avant les artefacts.
- Chaque étape a un schéma de sortie et des critères d'acceptation.
- Une étape = un appel LLM borné ; jamais un jeu entier en un JSON.

### 5.3 `AiOp` (patch atomique)

```rust
enum AiOp {
    CreateProject { .. },
    CreateScene { .. },
    AddEntity { .. },
    SetEntityComponent { .. },
    AddKvlNode { .. },
    ConnectPins { .. },
    SetVariable { .. },
    SetObjective { .. },
    AddLevel { .. },
    AddMeshInstance { .. },
    SetMaterial { .. },
    SetCamera3D { .. },
    AddLight3D { .. },
    GenerateSprite { .. },
    SetAudio { .. },
    SetUiScreen { .. },
    SetSaveConfig { .. },
    ExportTarget { .. },
}
```

- Chaque op a : schéma, validation sémantique, handler d'application, inverse pour rollback.
- Application sur un **clone** du projet ; commit atomique seulement si toutes les validations passent.
- `AiPatch` existant devient un **lot d'ops 2D** (compatibilité), pas le contrat final.

### 5.4 Contrat Studio (Tauri)

L'IA ne doit pas être une modale « génère et débrouille-toi ». Le Studio expose :

```text
ai_start(intent) -> run_id
ai_plan(run_id) -> AiPlan            (affiché avant génération)
ai_step(run_id, step_id) -> progress (streaming/annulation)
ai_review(run_id) -> diff + rapports + captures
ai_apply(run_id) / ai_rollback(run_id)
ai_history(project_id)
```

UI minimale : intention → plan (étapes, artefacts, acceptance) → progression → revue
(diff, validations, captures) → appliquer/annuler. Chaque étape est inspectable.

---

## 6. Validation et réparation

| Étage | Vérifie | Échec → |
|---|---|---|
| Schéma | JSON Schema/`serde` | réparation ciblée |
| Sémantique | IDs, refs, dimensions, bornes, assets | réparation |
| Catalogue | `NodeRegistry::validate_graph`, actions/conditions/événements | rejet/réparation |
| Sandbox | topologie KVL, 100 pas, pas de panique | rejet |
| Playtest | victoire/défaite atteignables, joueur peut se déplacer | réparation |
| Visuel | rendu 1200×800, absence de chevauchement, diff | réparation |
| Assets | PNG/SVG/mesh décodables, licences | rejet |
| Déterminisme | même entrée + seed → même hash de patch | avertissement |
| Sécurité | ops allowlist, chemins, pas de shell | rejet |

**Boucle de réparation** :
1. le validateur renvoie une liste structurée `{code, path, message, expected}` ;
2. le prompt de réparation ne contient que **l'étape fautive**, son schéma et les erreurs ;
3. maximum 2 tentatives ; ensuite fallback déterministe (template de niveau) ou abandon ;
4. chaque tentative est enregistrée dans `AiRun`.

---

## 7. Spécificités 2D et 3D

### 7.1 2D

- Playbooks : plateforme, RPG, shoot'em up, puzzle/Sokoban, boss, éducatif.
- Artefacts : terrain, entités/composants, KVL, sprites (archétype/ASCII/pxl), quêtes,
  dialogues, inventaire, HUD, audio.
- Contraintes : grille 32 px, gravité 900, vitesses 180-260, déterminisme 120 Hz.
- **Complétude L3** : plusieurs niveaux, progression, menus, sauvegarde, export.

### 7.2 3D / 2.5D

- **Déclaratif uniquement** : `Transform3D`, `Renderable3D`, `MeshRef`, `MaterialRef`,
  `Camera3D`, lumières. **Jamais** de code three.js/TSL généré.
- Assets : manifest de meshes (`mesh_assets`), primitives autorisées, matériaux limités.
- 2.5D d'abord : rendu 3D, collisions/physique 2D, caméra fixe ou orbitale.
- 3D complète seulement si la physique 3D existe ; sinon le plan l'interdit explicitement
  (gate de capacité) au lieu de laisser le LLM promettre l'impossible.
- Playbooks : scène 3D, îlot, donjon 2.5D, arène de boss.
- Validation : `RenderPacket` décodable, caméra dans les bornes, pas de mesh manquant,
  échelle/rotations finies.

### 7.3 Passage 2D ↔ 3D

- `dimension` explicite dans l'intention et dans chaque scène.
- Même modèle d'entités/ IDs ; le rendu et la physique changent, pas le contrat de jeu.
- 2.5D = même scène, `transform3d` + collision 2D ; le LLM ne doit pas mélanger les deux
  sans le déclarer.

---

## 8. Fournisseurs et profils

| Profil | Contexte | Sortie | Pack |
|---|---|---|---|
| Ollama / LM Studio (petit) | 4-8 k | JSON strict, 1 artefact | L0-L3 + étape + 1 exemple |
| Claude / Gemini / DeepSeek | 32-200 k | JSON Schema + tools | L0-L5 + playbook complet |
| OfflineLocal | — | templates déterministes | aucun LLM |

- **JSON contraint** : `format` Ollama, `json_schema` LM Studio/OpenAI, tool use Claude.
- **Température 0 / seed** quand disponible ; sinon hash + validation.
- **Token budget** : estimation avant appel, troncature par priorité (constitution >
  schéma de l'étape > playbook > exemples > contexte).
- **Cache de prompt** (cloud) sur la partie statique du dictionnaire.
- **Streaming/annulation** : l'UI doit pouvoir arrêter une génération.
- **Fallback** : fournisseur local → cloud configuré → template déterministe.
- Remplacer progressivement `curl` par un client HTTP testable, ou garder `curl` mais
  derrière un trait `LlmBackend` pour les tests.

---

## 9. Traçabilité et sécurité

Chaque run écrit `.karacti/ai-runs/<id>.json` :

```json
{
  "run_id": "run_01J...",
  "dictionary_version": "2026.11.1",
  "provider": "lmstudio",
  "model": "nvidia/nemotron-3-nano-4b",
  "seed": 42,
  "prompt_hash": "sha256:…",
  "plan": { },
  "steps": [
    { "step": "s1", "patch_hash": "…", "validation": "passed", "attempts": 1 }
  ],
  "applied": true,
  "screenshots": ["…"],
  "rollback": null
}
```

- **Injection de prompt** : les documents projet sont des données, jamais des instructions ;
  les chunks sont étiquetés `trusted: false` et ne peuvent pas modifier les règles.
- **Allowlist d'ops** : pas de shell, pas d'écriture hors projet, pas de réseau non déclaré.
- **Assets** : vérifier licence/provenance avant intégration.
- **Cloud** : opt-in explicite, pas de secrets dans le prompt, journaliser l'envoi.
- **Coût** : budget tokens par run, coupure automatique.

---

## 10. Évaluation

### 10.1 Corpus

- 20 intentions 2D (plateforme, RPG, shmup, puzzle, boss, éducatif).
- 10 intentions 3D/2.5D (scène, arène, donjon).
- 10 intentions de modification (« ajoute un checkpoint »).
- Chaque cas : intention, artefacts attendus, assertions, capture de référence.

### 10.2 Métriques

| Métrique | Cible |
|---|---|
| Validité schéma au 1er essai | ≥ 95 % |
| Validité après réparation | 100 % |
| Taux d'application (ops) | ≥ 90 % |
| Sandbox + playtest | ≥ 95 % |
| Couverture de la matrice L3 2D | 100 % |
| Couverture L2 3D | 100 % |
| Déterminisme (temp 0) | 100 % de patches identiques |
| Tokens prompt local | ≤ 8 k |
| Tokens prompt cloud | ≤ 32 k |
| Latence p50 par étape | local ≤ 30 s, cloud ≤ 10 s |
| Acceptation humaine (30 prompts) | ≥ 80 % |
| Code/TSL généré | **0** |

### 10.3 A/B testing

- Ancien prompt monolithique vs dictionnaire compilé ;
- avec/sans exemples dorés ;
- avec/sans retrieval ;
- petit modèle vs cloud.
Objectif : prouver que le gain vient du **contrat + réparation**, pas de la taille du prompt.

---

## 11. Roadmap

| Phase | Durée | Livrable | Critère de sortie |
|---|---|---|---|
| **0 — Audit & gel** | 2 j | inventaire, matrice de capacités, décisions D0-D5 | matrice validée, spec 60/120 corrigée |
| **1 — Compilateur de dictionnaire** | 3-4 j | `ai_dictionary` + `dist/ai/v1` + loader + tests de dérive | 0 divergence schémas/nœuds |
| **2 — Assembleur & profils** | 3-4 j | prompt par étapes, budgets, profils providers, snapshots | prompts reproductibles, budget respecté |
| **3 — Intention & plan** | 4-5 j | `IntentEnvelope`, clarification, `AiPlan`, schémas | plan valide sur 30 intentions |
| **4 — Boucle 2D** | 5-6 j | ops 2D, réparation, sandbox, playtest, visuel | L2 2D ≥ 90 %, L3 partiel |
| **5 — 3D / 2.5D** | 5-6 j | schémas/playbooks 3D, caméra/lumières/mesh, gate | L2 3D ≥ 90 % |
| **6 — Jeu complet** | 6-8 j | projet, multi-niveaux, UI, audio, sauvegarde, export | L3 2D sur 5 archétypes |
| **7 — Autonomie & CI** | 5 j | playtest bot, réparation auto, eval CI, télémétrie | L4 sur corpus restreint |

**Chemin critique** : 1 → 2 → 3 → 4 → 6. Les phases 5 et 7 peuvent démarrer en parallèle
après la phase 3.

---

## 12. Premier sprint (5-7 jours) — sans changer le comportement du LLM

Objectif : poser le contrat, mesurer, ne pas encore « faire confiance ».

1. **Créer `ai/`** :
   - `constitution.md`, `capabilities.yaml`, `glossary.md` ;
   - `playbooks/2d-platformer.md`, `playbooks/3d-scene.md` ;
   - 3 exemples golden.
2. **`crates/engine-core/src/ai_dictionary.rs`** :
   - types `DictionaryManifest`, `DictionaryLayer`, `DictionaryError` ;
   - `build()` + `load()` + hashes + compatibilité API.
3. **Binaire `ai_dictionary`** :
   - schémas `schemars` (`level`, `project`, `scene`, `graph`, `ops`, `3d_scene`) ;
   - `nodes.json` depuis `PluginHost::catalog_json()` ;
   - `capabilities.json`, manifeste.
4. **`ai_prompt.rs`** :
   - `PromptAssembler` avec budget tokens et priorité ;
   - `ProviderProfile` (Ollama, LM Studio, Claude, autres) ;
   - `assemble(intent, step, context, dictionary)`.
5. **Brancher sans casser** : `get_system_prompt_for_provider` délègue au nouvel assembleur
   avec fallback sur l'ancien prompt ; aucun changement de génération.
6. **Tests** :
   - hashes et version ;
   - parité nœuds (75) ;
   - schémas = types Rust ;
   - budget local ≤ 8 k ;
   - 3 exemples 2D + 1 exemple 3D valides ;
   - prompt identique à entrée identique (snapshot).
7. **Mesure** : journaliser `prompt_hash`, `dictionary_version`, tokens estimés.

**Fichiers touchés** : `KARACTI_AI_SPEC.md` (déprécié comme source), `ai_client.rs`,
`ai_dictionary.rs`, `ai_prompt.rs`, `Cargo.toml` (bin), `scripts/ai_dictionary.sh`,
`docs/PLAN_INTENTION_IA_RAG.md`, `dist/ai/`.

---

## 13. Risques

| Risque | Impact | Mitigation |
|---|---|---|
| Dérive doc/code | Élevé | dictionnaire généré + CI de non-dérive |
| Contexte trop long | Élevé | packs par étape, budgets, retrieval |
| Petit modèle JSON instable | Élevé | schéma contraint, 1 artefact/appel, réparation |
| Injection de prompt | Élevé | données non fiables, allowlist ops |
| 3D surpromise | Moyen | gate de capacité, 2.5D d'abord |
| Duplication CrewAI/Rust | Moyen | un seul dictionnaire, un seul contrat |
| Coût cloud | Moyen | budget, cache, local-first |
| Non-déterminisme | Moyen | temp 0/seed, hash, replay |
| Assets/licences | Moyen | validation de provenance |

---

## 14. Décisions ouvertes

- **D0 — Périmètre** : construire un **jeu** avec l'IA (hypothèse de ce plan) ou le
  **moteur** lui-même ? La réponse change l'échelle d'un facteur 10.
- **D1 — 3D** : 2.5D d'abord (recommandé) ou 3D complète avec physique 3D ?
- **D2 — Complétude L3** : quels artefacts sont obligatoires pour la v1 ?
  (menus, sauvegarde, audio, export, multi-niveaux)
- **D3 — Retrieval** : lexical déterministe d'abord (recommandé) ou embeddings dès la v1 ?
- **D4 — Modèle local minimal** : 4B, 7B, 14B ? Quel contexte minimum ?
- **D5 — Cloud** : autorisé par défaut ou opt-in ? Quelles données peuvent sortir ?
- **D6 — Revue humaine** : obligatoire pour chaque patch (recommandé) ou seulement pour
  les opérations destructives ?

### Recommandations

| Décision | Recommandation | Raison |
|---|---|---|
| D0 | **Le jeu**, pas le moteur | Le moteur est un produit stable ; générer son code est un autre projet |
| D1 | **2.5D d'abord** | La physique 3D n'existe pas encore ; ne pas promettre l'impossible |
| D2 | **L3 2D** en v1, **L2 3D** | Menus/save/audio/export sont les vrais manques 2D |
| D3 | **Lexical d'abord** | Déterministe, zéro dépendance ; embeddings ensuite |
| D4 | **7B / 8k** minimum | Un 4B tient difficilement schéma + playbook + contexte |
| D5 | **Cloud opt-in** | Souveraineté et coût ; local par défaut |
| D6 | **Revue humaine** avant application | Boucle de confiance déjà présente dans le Studio |

---

## 15. Annexes

### A. Arborescence cible

```text
ai/
├── constitution.md
├── capabilities.yaml
├── glossary.md
├── playbooks/
│   ├── 2d-platformer.md
│   ├── 2d-rpg.md
│   ├── 2d-boss.md
│   └── 3d-scene.md
└── examples/
    ├── 2d-platformer-boss.json
    └── 3d-arena.json

dist/ai/current/
├── manifest.json
├── schemas/*.schema.json
├── nodes.json
├── capabilities.json
└── playbooks/*.md

crates/engine-core/src/
├── ai_dictionary.rs
├── ai_intent.rs
├── ai_prompt.rs
├── ai_plan.rs
├── ai_ops.rs
├── ai_client.rs      (existant, adapté)
└── ai_patch.rs       (existant, devient un lot d'ops 2D)
```

### B. Commandes

```bash
cargo run -p engine-core --bin ai_dictionary -- --out dist/ai/current
cargo test -p engine-core ai_dictionary
cargo test -p engine-core ai_prompt
cargo test --workspace
```

### C. Migration de `KARACTI_AI_SPEC.md`

- Le fichier reste la **référence humaine** (vulgarisation, pédagogie).
- Il n'est plus la source du prompt : les sections machine sont générées dans `dist/ai`.
- Ajouter un bandeau en tête : « Document humain — la source machine est `dist/ai/current` ».
- Corriger les dérives connues (60 → 120 Hz, nœuds réels, 3D) lors de la phase 0.
- Le prompt cloud peut encore l'inclure en **lecture seule**, mais jamais à la place du
  schéma de l'étape.

### D. Règles d'or

1. **Le code est la vérité**, le dictionnaire est généré.
2. **Un appel = un artefact** ; le plan est petit.
3. **Aucune op invalide n'atteint le projet** : clone + validation + commit atomique.
4. **Une action inconnue est une erreur**, jamais un no-op silencieux.
5. **Pas de code généré** : uniquement des données/ops déclaratives.
6. **Toujours mesurer** : version, hash, tokens, latence, validation, acceptation.
7. **Le LLM propose, le moteur dispose.**
