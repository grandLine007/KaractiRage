# Plan v2 — Intention directe : modifier chaque élément à la volée (2D / 3D)

> **Version** : 2.0 — 3 novembre 2026
> **Complément de** : [PLAN_INTENTION_IA_RAG.md](PLAN_INTENTION_IA_RAG.md)
> **Objectif** : passer d'une IA « générateur de niveau dans une modale » à une IA
> **contextuelle, ancrée sur la sélection**, qui modifie n'importe quel élément du jeu
> sans quitter le viewport, l'inspecteur ou le graphe KVL.
> **Périmètre** : jeu/projet produit par le moteur, 2D, 2.5D et 3D déclarative.
> **Statut** : plan de conception — à valider avant implémentation.

---

## 0. Résumé exécutif

Le socle du plan v1 est déjà largement implémenté : dictionnaire compilé, intention,
plan, opérations, exécuteur atomique, prompt budgété, streaming, historique, revue
visuelle et commandes Tauri. **344 tests passent.** Le problème n'est plus « générer un
niveau », c'est **éditer le bon élément, au bon endroit, sans changer de contexte**.

### Les 8 décisions structurantes

1. **La sélection est le prompt.** L'IA reçoit automatiquement l'élément sélectionné,
   son type, ses valeurs, ses voisins et la caméra — pas une scène entière.
2. **Deux vitesses.** Micro-édition (1 op, inline, sans plan) et macro-construction
   (plan multi-étapes). Le plan actuel ne couvre que la seconde.
3. **Pas de page IA.** Une barre ancrée, un bouton sur la sélection, un « ✦ » sur les
   champs : le viewport reste visible et interactif.
4. **Des ops de mutation, pas seulement des ops de création.** Le contrat actuel sait
   créer un niveau ; il ne sait pas encore déplacer, supprimer, colorer, renommer,
   paramétrer ou reconnecter un élément existant.
5. **Un seul pipeline.** `ai_step` (payload de niveau) et `ai_plan`/`ai_executor`
   (ops) sont deux chemins parallèles ; il faut les unifier derrière `EditProposal`.
6. **Preview + undo par intention.** Ghost visuel, diff sémantique, `Entrée` applique,
   `Échap` annule, `⌘Z` défait le groupe.
7. **La validation avant l'application.** Cible résolue, schéma, types de pins,
   catalogue KVL, bornes, assets, dimensions ; une op invalide est une erreur, jamais
   un no-op.
8. **Le LLM propose, le moteur dispose.** Aucun code three.js/TSL généré ; uniquement
   des ops déclaratives validées.

---

## 1. Audit de l'existant

### 1.1 Ce qui est déjà solide

| Brique | Fichier | État |
|---|---|---|
| Dictionnaire compilé (schémas, 75 nœuds, playbooks, exemples) | `crates/engine-core/src/ai_dictionary.rs` | ✅ manifeste + hashes |
| Normalisation d'intention | `ai_intent.rs` | ✅ cible/dimension/genre + clarification |
| Plan + ops | `ai_plan.rs` | ✅ 18 ops, dépendances, cycles |
| Exécuteur atomique | `ai_executor.rs` | ✅ clone, rollback, erreurs explicites |
| Prompt budgété + retrieval lexical | `ai_prompt.rs` | ✅ profils local/cloud |
| Streaming + annulation | `ai_stream.rs`, Tauri `ai_step` | ✅ SSE + cancel |
| Traçabilité des runs | `ai_history.rs` | ✅ `.karacti/ai-runs` |
| Revue visuelle | `preview.rs`, `ai_review` | ✅ diff de capture |
| UI existante | `AiAssistantModal.tsx` | ✅ providers, plan, streaming |

### 1.2 Les écarts avec « modifier à la volée »

| # | Écart | Preuve / impact | Sévérité |
|---|---|---|---|
| 1 | `ai_step` ignore `AiPlan`/`AiOp` et renvoie un `AiGeneratedGamePayload` | `ai_orchestrator.rs`, `apps/studio/src-tauri/src/lib.rs` | **Bloquant** |
| 2 | Aucune op de mutation : pas de `UpdateEntity`, `DeleteEntity`, `Move`, `SetProperty` | `AiOp` dans `ai_plan.rs` | **Bloquant** |
| 3 | Les cibles sont des noms, pas des IDs stables | `SetEntityComponent { entity: String }`, `SetMaterial` | **Élevé** |
| 4 | Aucun contexte de sélection n'est envoyé au prompt | `assemble_full(..., None, None)` dans `ai_step` | **Bloquant** |
| 5 | L'IA est une modale plein écran | `AiAssistantModal.tsx`, `I` dans `App.tsx` | **Élevé** |
| 6 | Le prompt utilise `SCHEMA_HINT` manuel, pas le schéma généré | `ai_prompt.rs` vs `dist/ai/current/schemas/` | **Élevé** |
| 7 | La revue est un diff d'image global, pas un diff d'op | `review_ai_patch` / `preview_diff` | **Moyen** |
| 8 | Les ops existantes créent à `(0,0)` et ne portent pas de transform | `AddEntity`, `AddMesh` | **Élevé** |
| 9 | `ConnectPins` ne valide ni l'existence ni le type des pins | `ai_executor.rs` | **Élevé** |
| 10 | `SetAudio` est déclaré mais non supporté | test `unsupported_op_is_an_error_not_a_silent_noop` | **Moyen** |
| 11 | Pas de propriétés éditables par élément (vitesse, friction, couleur, tags…) | inspecteur actuel | **Élevé** |
| 12 | Pas de multi-sélection, batch, zone, « ici » | aucune notion de `TargetRef` | **Élevé** |
| 13 | Pas de tool calling / MCP pour les agents externes | aucune surface outils | **Moyen** |
| 14 | Pas de benchmark d'édition | tests unitaires seulement | **Moyen** |

### 1.3 Le vrai problème

Le moteur sait **construire** un niveau à partir d'un JSON ; il ne sait pas encore
**éditer** un projet existant élément par élément. La différence est structurelle :

- construire = créer des objets à partir de zéro ;
- éditer = résoudre une cible, capturer son état, proposer un delta minimal, le valider,
  le prévisualiser, l'appliquer et pouvoir l'annuler.

C'est cette chaîne **cible → delta → validation → preview → undo** qui manque.

---

## 2. Principe : « la sélection est le prompt »

### 2.1 Les trois questions de l'IA inline

Toute demande inline doit résoudre trois choses :

1. **Quoi ?** → la cible (`TargetRef`) : sélection, survol, curseur, zone, tag.
2. **Quoi changer ?** → un delta typé (`EntityPatch`, `KvlPatch`, `ScenePatch`).
3. **Est-ce acceptable ?** → validation + preview + risque.

Le LLM ne doit jamais deviner la cible à partir d'une scène entière. Le Studio la lui
fournit, et affiche la cible résolue à l'utilisateur.

### 2.2 Deux vitesses, un seul contrat

| Mode | Déclencheur | Étapes | Latence cible | Exemple |
|---|---|---|---|---|
| **Micro** | `⌘K` / `I` / ✦ sur sélection | 1 appel → 1 op → preview | < 2 s (template), < 15 s (local) | « mets ce boss en rouge » |
| **Macro** | `⌘⇧K` / « Créer… » | intention → plan → N ops | 10-60 s | « crée un niveau 2D avec un boss » |
| **Propriété** | ✦ sur un champ | 1 appel → 1 valeur | < 5 s | « plus rapide » → speed 260 |
| **Batch** | sélection multiple | 1 op batch | < 15 s | « supprime ces 3 pièges » |

Le mode macro réutilise le plan/executor existant. Le mode micro est nouveau et
**ne doit pas passer par un plan**.

---

## 3. UX : aucune page, quatre surfaces

### 3.1 Surface A — affordance sur la sélection

Quand un élément est sélectionné (entité, nœud KVL, mesh 3D) :

- un petit bouton `✦` apparaît à côté de la sélection (ou dans la barre d'outils) ;
- `⌘K` (ou `I`) ouvre une **barre ancrée** ;
- un raccourci `⌘⇧K` ouvre le mode « construire » ;
- `Échap` ferme sans rien appliquer.

Pour le canvas 2D/3D, ancrer la barre au rectangle écran de la sélection ; à défaut,
une barre flottante en bas du viewport, avec un chip de cible explicite.

**Ancrage** :
- 2D : `world → screen` via la caméra du moteur, clamp dans le viewport ;
- 3D : projection du `Transform3D` avec la matrice caméra ; si l'élément est derrière
  la caméra, repli sur la barre basse ;
- KVL : position du nœud React Flow ;
- sans sélection : barre basse avec cible = survol/curseur, ou sélecteur d'élément.

### 3.2 Surface B — barre d'édition ancrée

```text
┌─────────────────────────────────────────────────────────────┐
│ ✦  [Cible : Boss #42 ×]  [Scène : niveau_1]  [2D]           │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Rends ce boss plus rapide et rouge                    ↵ │ │
│ └─────────────────────────────────────────────────────────┘ │
│  Suggestions :  Plus rapide · Rouge · Dupliquer · Supprimer │
│  ─────────────────────────────────────────────────────────  │
│  Proposition : vitesse 110 → 160 · couleur #DC2626           │
│  [Aperçu]  [Appliquer ↵]  [Annuler Échap]                    │
└─────────────────────────────────────────────────────────────┘
```

- Pas de backdrop, pas de plein écran.
- Le viewport reste visible ; le ghost de prévisualisation est dessiné dans le canvas.
- Les chips de contexte sont **éditables** : changer la cible sans reformuler.
- Les suggestions sont déterministes et contextuelles (pas un menu générique).

### 3.3 Surface C — propriété « ✦ »

Chaque champ éditable de l'inspecteur (position, taille, vitesse, couleur, santé,
tags, matériau, mesh, lumière) reçoit un petit bouton `✦` :

- clic → popover d'une ligne (« plus rapide », « couleur sang », « 2× plus grand ») ;
- proposition → valeur + unité + bornes ;
- appliquer → champ mis à jour, undo groupé.

C'est le point d'entrée le plus **précis** et le plus rapide pour « modifier chaque
élément » : pas besoin de décrire l'élément, il est déjà ciblé par le champ.

### 3.4 Surface D — palette globale / outil IA

- `⌘K` sans sélection : palette contextuelle (cible = survol/curseur, sinon demande).
- Outil « IA » dans la barre : clic sur un élément = cible ; glisser un rectangle =
  zone multi-sélection.
- `⌘L` : bascule vers l'assistant complet (macro), sans perdre le contexte.
- La modale actuelle **reste** : elle devient l'« Atelier IA » pour le jeu complet, les
  plans longs et les réglages de fournisseur. L'inline est le chemin par défaut.
- Voix push-to-talk : option phase 4, pas un prérequis.

### 3.5 Preview, diff, undo

- **Ghost** : entité dessinée en violet semi-transparent à la nouvelle position /
  couleur / échelle ; pour KVL, nœuds et arêtes fantômes.
- **Diff sémantique** : liste `champ: avant → après`, pas un diff JSON brut.
- **Application** : `Entrée` ou bouton ; **annulation** : `Échap`.
- **Undo** : `⌘Z` défait le groupe entier (une intention = une transaction).
- **Risque** : vert (sûr), ambre (à confirmer), rouge (destructif), comme dans les
  plugins d'édition agentique.

### 3.6 Règles d'interaction (HAX + pédagogie KaractiRage)

| Règle | Application |
|---|---|
| G1 — montrer ce que l'IA sait faire | suggestions contextuelles, pas un champ vide |
| G4 — information contextuelle | chips cible/scène/dimension |
| G7 — invocation efficace | `⌘K`, `I`, ✦, clic droit |
| G8 — rejet efficace | `Échap`, clic extérieur, bouton Annuler |
| G9 — correction efficace | « Reformuler », changer la cible, éditer la proposition |
| G11 — expliquer pourquoi | phrase « Je vais modifier X car… » + diff |
| G16 — conséquences | risque, nombre d'éléments, perte de données |
| G18 — notifier | toast « 3 modifications appliquées · ⌘Z » |

Le langage reste **français, concret, sans jargon** ; les noms techniques KVL sont
accompagnés de leur sens (« QUAND », « SI », « ALORS »).

---

## 4. Architecture cible

```text
Sélection / survol / curseur
        │
        ▼
  EditContext (Rust) ──► Prompt micro (1-3 k tokens)
        │                         │
        │                         ▼
        │                 LLM / template / outil
        │                         │
        ▼                         ▼
  ResolveTarget ──────────► EditProposal (1 op)
                                  │
                                  ▼
                    Validation (schéma, cible, types, bornes)
                                  │
                                  ▼
                    Preview (ghost + diff sémantique)
                                  │
                     ┌────────────┴────────────┐
                     ▼                         ▼
                 Appliquer                   Annuler
                     │
                     ▼
              Transaction + undo groupé
```

### 4.1 `EditContext`

```rust
pub struct EditContext {
    pub project_id: String,
    pub scene_id: String,
    pub dimension: Dimension,          // 2d | 2.5d | 3d
    pub selection: Vec<TargetRef>,
    pub hover: Option<TargetRef>,
    pub cursor_world: Option<[f32; 3]>,
    pub active_panel: ActivePanel,     // viewport | inspector | kvl | 3d
    pub active_tool: String,
    pub camera: Option<Camera3DData>,
    pub recent_ops: Vec<String>,
}
```

Sérialisation **compacte** pour le prompt : uniquement les champs utiles, jamais la
scène complète. Les entités sont résumées par `id`, `name`, `tag`, `transform`,
`sprite`, `components`.

### 4.2 `TargetRef`

```rust
pub enum TargetRef {
    Selected,
    Hovered,
    EntityId(u32),
    EntityName(String),
    KvlNodeId(String),
    Cursor { x: f32, y: f32, z: Option<f32> },
    Area { x: f32, y: f32, w: f32, h: f32 },
    Tag(String),
    AllOfKind(String),
}
```

Résolution stricte : si `Selected` ne contient aucun élément, l'IA demande une cible
au lieu d'inventer. Si plusieurs éléments, afficher « 3 éléments » et un preview batch.

### 4.3 `EditProposal`

```rust
pub struct EditProposal {
    pub id: String,
    pub intent_id: String,
    pub target: ResolvedTarget,        // IDs réels, libellés humains
    pub ops: Vec<AiOp>,                // 1 pour micro, N pour batch
    pub rationale: String,             // « Je vais… »
    pub risk: RiskLevel,               // safe | confirm | destructive
    pub validation: ValidationReport,
    pub preview: PreviewDelta,         // avant/après par champ
    pub provider: String,
    pub model: String,
    pub tokens: TokenUsage,
}
```

`EditProposal` est **immuable** et hashée. Appliquer un proposal périmé (cible
supprimée entre-temps) échoue proprement.

### 4.4 Extension d'`AiOp`

Les ops existantes couvrent la création. Ajouter :

```rust
UpdateEntity { target: TargetRef, patch: EntityPatch },
DeleteEntity { targets: Vec<TargetRef> },
DuplicateEntity { targets: Vec<TargetRef>, offset: [f32; 2] },
SetTransform2D { target: TargetRef, x: f32, y: f32, w: f32, h: f32 },
SetTransform3D { target: TargetRef, position, rotation, scale },
SetEntityProperty { target: TargetRef, path: String, value: PropertyValue },
AddComponent { target: TargetRef, component: String },
RemoveComponent { target: TargetRef, component: String },
SetKvlNodeParam { target: TargetRef, param: String, value: PropertyValue },
DeleteKvlNode { target: TargetRef },
RemoveConnection { id: String },
SetSceneSetting { key: String, value: PropertyValue },
Batch { ops: Vec<AiOp> },
```

`EntityPatch` doit être **partiel** (seuls les champs modifiés) et **typé** :

```rust
pub struct EntityPatch {
    pub name: Option<String>,
    pub tag: Option<String>,
    pub color: Option<u32>,
    pub x: Option<f32>, pub y: Option<f32>,
    pub width: Option<f32>, pub height: Option<f32>,
    pub speed: Option<f32>,
    pub health: Option<f32>,
    pub sprite: Option<String>,
    pub mesh: Option<String>,
    pub material: Option<MaterialPatch>,
    pub tags: Option<Vec<String>>,
}
```

### 4.5 Transaction et undo

- `apply_proposal` applique sur un **clone** ;
- capture `before` des objets touchés ;
- si validation OK → commit dans le projet + push `AiTransaction` dans l'undo ;
- `undo` restaure les `before` et retire le groupe ;
- une intention = une transaction = un `⌘Z` ;
- pas d'écriture disque avant commit.

### 4.6 Commandes Tauri

```text
ai_context() -> EditContext
ai_edit_preview(context, instruction) -> EditProposal
ai_edit_apply(proposal_id) -> ApplyReport
ai_edit_cancel(proposal_id)
ai_edit_undo(transaction_id)
ai_edit_stream(run_id) -> events (progress, proposal, error)
ai_plan_preview(prompt, context) -> AiPlan
ai_plan_apply(plan_id) -> ApplyReport
```

`ai_edit_preview` ne modifie **rien** : il renvoie une proposition. C'est la clé de
la boucle de confiance.

---

## 5. Prompt micro : petit, ciblé, contraint

### 5.1 Blocs

```text
[CONSTITUTION]   invariants, pas de code, 1 op, JSON brut
[OP SCHEMA]      schéma réel de l'op (extrait de dist/ai/current/schemas/ops.schema.json)
[TARGET]         id, nom, tag, transform, composants, valeurs actuelles
[NEIGHBORS]      3-5 voisins utiles (pas la scène entière)
[CAPABILITIES]   propriétés éditables pour ce type, bornes
[EXAMPLES]       1 exemple micro (instruction → op)
[INSTRUCTION]    demande utilisateur
[OUTPUT]         EditProposal JSON
```

Budget : **1,5-3 k tokens** en local, **4-8 k** en cloud. Le schéma complet n'est
jamais injecté ; on extrait l'op pertinente.

### 5.2 Résolution rapide (sans LLM)

Avant l'appel, un **résolveur déterministe** couvre les demandes fréquentes :

| Formulation | Op |
|---|---|
| « rouge », « bleu », « couleur X » | `UpdateEntity { color }` |
| « plus rapide / plus lent » | `UpdateEntity { speed }` |
| « plus grand / plus petit » | `SetTransform` / scale |
| « duplique » | `DuplicateEntity` |
| « supprime » | `DeleteEntity` (risque destructif) |
| « déplace ici / à X,Y » | `SetTransform` (curseur) |
| « renomme en X » | `UpdateEntity { name }` |
| « aligne / répartis » | `Batch` |
| « ajoute une lumière » | `AddLight` (position curseur) |
| « rends métallique » | `SetMaterial` |

Objectif : **60-80 % des micro-éditions sans LLM**, latence < 100 ms, zéro
hallucination. Le LLM reste pour le freeform, le compound et l'ambigu.

### 5.3 Sortie contrainte

- Ollama : paramètre `format` avec JSON Schema ([doc](https://registry.ollama.com/blog/structured-outputs)) ;
- LM Studio : `response_format: json_schema` ([doc](https://lmstudio.ai/docs/developer/openai-compat/structured-output)) ;
- Claude : tool use / structured output ;
- fallback : parse + repair + rejet.

**Attention** : LM Studio documente que les modèles **< 7B** sont souvent incapables
de structured output fiable. C'est un argument pour un minimum 7B/8k (voir §12).

---

## 6. Validation et preview

| Étape | Contrôle |
|---|---|
| Cible | exactement 1 (ou N pour batch), type compatible, pas supprimée |
| Schéma | `serde` / JSON Schema de l'op |
| Propriétés | champ connu, type, bornes, unité |
| KVL | `NodeRegistry::resolve`, pins existants, types compatibles, pas de cycle |
| 3D | mesh présent dans le manifest, transform fini, matériau connu |
| Dimension | 2D ↔ 3D cohérent |
| Risque | suppression/écrasement = confirmation |
| Preview | ghost + diff, région limitée à la cible |
| Déterminisme | hash du proposal identique à contexte/instruction égaux |

Une op inconnue ou une cible ambiguë **échoue avec une explication**, jamais un no-op.

---

## 7. 2D, 3D et KVL

### 7.1 2D

- Sélection dans le viewport → entité.
- Barre ancrée + ✦ dans l'inspecteur.
- Propriétés : position, taille, couleur, sprite, vitesse, santé, tags, collider.
- « ici » = coordonnées monde sous le curseur.
- Batch : zone, tag, type.

### 7.2 3D / 2.5D

- Sélection via gizmo / clic ; raycast pour « ici ».
- Propriétés : `position3d`, `rotation_euler_deg`, `scale3d`, `mesh_asset`,
  `material`, lumière, caméra.
- Preview : ghost du mesh transformé, pas de code.
- 2.5D : mêmes ops, collision 2D ; la dimension est explicite.
- Gate : si le moteur ne sait pas faire (physique 3D complète), l'IA le dit.

### 7.3 KVL

- Sélection d'un nœud → « ajoute une condition », « relie à… », « change le seuil ».
- Preview : nœuds/arêtes violets, validation des pins avant insertion.
- Ops : `SetKvlNodeParam`, `DeleteKvlNode`, `RemoveConnection`, `Batch`.
- Le graphe est un **graphe**, pas du texte : pas de diff textuel, des ops typées.

---

## 8. Veille technologique et open source

> Les sources externes sont des références ; elles ne dictent pas le projet.

### 8.1 Édition inline et UX

| Source | Apport | Usage KaractiRage |
|---|---|---|
| [VS Code — Inline chat](https://code.visualstudio.com/docs/chat/inline-chat) | La sélection scope le prompt ; diff inline ; **Keep / Undo** | Modèle direct de la barre ancrée et du preview |
| [Cursor — Inline Edit](https://cursor.com/changelog/0-50) | `⌘K` édition inline, `⌘⇧⏎` full file, `⌘L` agent | `⌘K` micro, `⌘⇧K` macro, `⌘L` assistant |
| [Microsoft HAX Guidelines](https://www.microsoft.com/en-us/haxtoolkit/ai-guidelines/) | 18 règles human-AI (G1, G4, G7-G9, G11, G16, G18) | Chips de contexte, explicabilité, annulation, conséquences |
| [Anthropic — Context engineering](https://www.anthropic.com/research/effective-context-engineering-for-ai-agents) | « context rot » ; curer le contexte | Pack de sélection, pas la scène entière |

### 8.2 Sorties structurées et modèles locaux

| Source | Apport | Usage |
|---|---|---|
| [Ollama — Structured outputs](https://registry.ollama.com/blog/structured-outputs) | `format` = JSON Schema ; température 0 | Micro-ops locales |
| [LM Studio — Structured Output](https://lmstudio.ai/docs/developer/openai-compat/structured-output) | `response_format` JSON Schema ; **< 7B peu fiable** | Profil local 7B+ |
| [Outlines](https://github.com/dottxt-ai/outlines) | Génération structurée garantie | Sidecar / recherche |
| [XGrammar](https://github.com/mlc-ai/xgrammar) | Décodage contraint, overhead quasi nul | Backends vLLM/SGLang |
| [llguidance](https://github.com/guidance-ai/llguidance) | **Rust**, JSON Schema, ~50 µs/token | Option backend local intégré |
| [Aider — Edit formats](https://aider.chat/docs/more/edit-formats.html) | whole / diff / udiff | À **ne pas** utiliser pour la scène : ops typées |

### 8.3 Éditeurs agentiques et outils

| Source | Apport | Usage |
|---|---|---|
| [Autonomix (Unreal)](https://github.com/PRQELT/Autonomix) | Boucle d'outils, checkpoints, **risk levels**, stop, résultats d'outils compacts | Modèle de transaction, risque, annulation |
| [xAssistant (Unreal)](https://github.com/hoemw/xAssistant) | Modification de propriétés par langage naturel, contexte de scène, batch, undo | Modèle du ✦ par champ et du batch |
| [MCP for Unity](https://github.com/CoplayDev/unity-mcp) | 48 outils, tout client MCP, scènes/assets/tests | Preuve qu'un catalogue d'outils éditeur fonctionne |
| [Godot AI](https://github.com/hi-godot/godot-ai) | Serveur MCP + outils pour moteur | Alternative open source à étudier |
| [MCP — Tools](https://modelcontextprotocol.io/specification/2026-07-28/server/tools.md) | Outils déclarés par schéma, **human-in-the-loop**, confirmation | Surface externe optionnelle ; pas le cœur inline |

### 8.4 Ce qu'il ne faut pas copier

- **Pas de diff textuel** pour une scène/un graphe : ops typées + undo.
- **Pas de code généré** (three.js/TSL) : données déclaratives uniquement.
- **Pas de dictionnaire entier** dans le prompt : retrieval + pack de sélection.
- **Pas de 4B** pour du structured output fiable : 7B/8k minimum.
- **Pas de chat plein écran** comme surface principale : inline d'abord.
- **Pas d'auto-application destructive** : preview + confirmation + undo.

---

## 9. Benchmark d'édition

### 9.1 Corpus micro

40 instructions × 10 cibles (entité, nœud KVL, mesh 3D) :

- couleur, vitesse, taille, position, nom, tag ;
- duplication, suppression, alignement, batch ;
- ajout de lumière, matériau, mesh ;
- condition KVL, connexion, seuil ;
- demandes ambiguës (« rends-le meilleur ») → clarification.

### 9.2 Métriques

| Métrique | Cible |
|---|---|
| Cible correcte | ≥ 95 % |
| Op valide au 1er essai | ≥ 95 % |
| Application réussie | ≥ 98 % |
| Latence médiane (template) | < 100 ms |
| Latence médiane (local 7B) | < 15 s |
| Taux d'annulation | < 10 % |
| Acceptation humaine | ≥ 85 % |
| Clics par édition | ≤ 3 |
| No-op silencieux | 0 |
| Code généré | 0 |

### 9.3 A/B

- Modale actuelle vs barre inline : temps de tâche, clics, erreurs.
- Template vs LLM : latence, précision.
- Avec/sans contexte de sélection : taux de bonne cible.
- 7B vs cloud : qualité/coût/latence.

---

## 10. Roadmap

### Sprint 1 — Socle micro-édition (1 semaine)

- `ai_edit.rs` : `EditContext`, `TargetRef`, `EditProposal`, `RiskLevel`.
- Étendre `AiOp` : `UpdateEntity`, `DeleteEntity`, `DuplicateEntity`, `SetTransform2D`,
  `SetEntityProperty`, `AddComponent`, `RemoveComponent`, `SetKvlNodeParam`,
  `DeleteKvlNode`, `RemoveConnection`, `Batch`.
- `resolve_target` + `preview_edit` + `apply_edit` + `undo` sur clone.
- Prompt micro + template resolver.
- Commandes Tauri `ai_context`, `ai_edit_preview`, `ai_edit_apply`, `ai_edit_cancel`.
- Tests : 10 instructions, cible, validation, rollback, no-op.

### Sprint 2 — UI inline 2D (1 semaine)

- `InlineAiBar.tsx` : barre ancrée, chips, suggestions, diff, apply/cancel.
- `AiSparkle` dans `EntityInspector`.
- `⌘K` / `I` / `Échap` / `Entrée`.
- Ghost de prévisualisation dans le canvas.
- Undo groupé + toast.
- A11y : focus, aria-live, 24px, contraste.

### Sprint 3 — KVL + batch + 3D (1 semaine)

- Barre inline dans `KvlGraphEditor` (nœud sélectionné).
- Batch multi-sélection et zone.
- 3D : gizmo, curseur, matériau/lumière/mesh, ghost.
- Historique IA dans la barre de statut.

### Sprint 4 — Agentique et externe (optionnel)

- Tool calling pour le macro (cloud) ; MCP server pour Claude Desktop/Cursor.
- Voix push-to-talk.
- Évaluation automatisée + télémétrie.

---

## 11. Risques

| Risque | Mitigation |
|---|---|
| Cible ambiguë | chips + résolution stricte + clarification |
| Latence locale | templates, warmup, streaming, annulation |
| Édition trop large | delta minimal, diff, preview |
| Op destructive | risk level, confirmation, undo |
| Hallucination de champ | schéma réel + propriétés éditables |
| Prompt injection | documents projet non fiables, allowlist |
| Conflit édition manuelle | transaction, hash de cible, apply périmé refusé |
| Surcharge UI | une seule affordance, progressive disclosure |
| 3D impossible | capability gate, 2.5D d'abord |

---

## 12. Décisions ouvertes

- **D1 — Micro par défaut ou preview systématique ?** Recommandation : preview pour
  medium/destructive, application directe + undo pour safe.
- **D2 — 7B minimum ?** Recommandation : oui, 8k de contexte.
- **D3 — Templates avant LLM ?** Recommandation : oui, 60-80 % des micro-éditions.
- **D4 — Tool calling/MCP en v1 ?** Recommandation : plus tard ; micro = 1 op.
- **D5 — Portée 3D :** 2.5D d'abord, 3D complète selon la physique.
- **D6 — Où vit l'IA ?** Recommandation : Rust/Tauri pour le contrat et la validation ;
  React pour l'UI ; sidecar Python uniquement pour l'orchestration multi-agents.

---

## 13. Annexe — exemple de proposition

```json
{
  "id": "prop_01J...",
  "intent_id": "int_...",
  "target": {
    "kind": "entity",
    "id": 42,
    "label": "Boss_Squelette",
    "scene": "niveau_1"
  },
  "ops": [
    {
      "op": "update_entity",
      "target": { "entity_id": 42 },
      "patch": { "color": 14483750, "speed": 160.0 }
    }
  ],
  "rationale": "Le boss est ralenti et terne ; je propose une vitesse de 160 et un rouge rubis.",
  "risk": "safe",
  "validation": { "schema": "ok", "target": "ok", "properties": "ok" },
  "preview": {
    "changes": [
      { "field": "speed", "before": 110.0, "after": 160.0 },
      { "field": "color", "before": "#475569", "after": "#DC2626" }
    ]
  }
}
```

---

## 14. Fichiers cibles

```text
crates/engine-core/src/
├── ai_edit.rs          (nouveau : contexte, cible, proposition, transaction)
├── ai_plan.rs          (étendre AiOp)
├── ai_executor.rs      (résolution de cible, patch, undo)
├── ai_prompt.rs        (prompt micro, schéma réel, contexte)
└── ai_intent.rs        (cible « tweak » enrichie)

apps/studio/src/
├── components/InlineAiBar.tsx       (nouveau)
├── components/AiSparkle.tsx         (nouveau)
├── components/EntityInspector.tsx   (✦ par champ)
├── components/KvlGraphEditor.tsx    (barre inline)
├── components/GameViewport.tsx      (ghost, ancrage)
└── App.tsx                          (raccourcis, état)

apps/studio/src-tauri/src/lib.rs     (commandes ai_edit_*)
```

---

## 15. Règle finale

> **L'utilisateur ne doit jamais avoir à décrire ce qu'il voit.**
> Le Studio sait ce qui est sélectionné ; l'IA doit le savoir aussi.
