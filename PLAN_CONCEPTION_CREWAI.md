# 🧠 PLAN DE CONCEPTION — Orchestration agentique CrewAI (sidecar `agents/`)

> **Statut** : plan de conception v1 — 16 octobre 2026
> **Branche** : `feat/tauri-react-studio`
> **Objectif** : accélérer la conception (niveaux, scripts KVL, équilibrage, docs) en orchestrant
> plusieurs agents spécialisés via le **framework CrewAI**, sans casser l'architecture Rust
> ni la promesse « low-end first & déterministe ».
> **Référence vision** : `KARACTIRAGE_DESIGN_DOCUMENT_DARK.md` §3.3 (Agents IA & MCP),
> `KARACTI_AI_SPEC.md` (vérité terrain), `CRITIQUE_DESIGN_MI_PARCOURS.md` (Phase A/B/C IA).

---

## 1. Existant — ce que le plan ne doit pas réinventer

L'écosystème IA du projet est **déjà largement en place** côté Rust :

| Brique | Fichier | État | Rôle dans le plan |
|---|---|---|---|
| Spécification ground truth (schéma de niveau, catalogue KVL, règles sandbox §8) | `KARACTI_AI_SPEC.md` | ✅ v1.0.0 | **Contrat unique** côté Python → Pydantic le miroit |
| Client LLM multi-fournisseurs (Ollama, LM Studio, Claude, Gemini, DeepSeek, OpenAI, OfflineLocal) | `crates/engine-core/src/ai_client.rs` (870 l.) | ✅ | Configuration d'endpoint partagée (le sidecar vise le **même** endpoint) |
| Config active | `karactirage.ai.json` | ✅ LM Studio `127.0.0.1:1234`, `nemotron-3-nano-4b` | Cible locale par défaut du sidecar |
| Importeur LevelSpec → scène jouable (terrain, archétypes, win_condition) + tests | `crates/engine-core/src/ai_patch.rs` | ✅ | **Porte d'entrée** des artefacts CrewAI dans le jeu |
| Générateur déterministe recette → scène + résolveur de dépendances | `crates/engine-core/src/recipe.rs` (904 l.) | ✅ | Fallback sans LLM + générateur de scènes de test |
| Validateur statique KVL (typage des pins) | `crates/kvl-core` | ✅ | Garde-fou des graphes générés |
| Studio Tauri 2 + React 19 + xyflow | `apps/studio` | 🚧 | Surface UI du futur panneau « Assistant » |
| Guide KVL 3 étapes (wizard déterministe) | `kvl-core/src/guided.rs` + wizard Studio | ✅ | Socle « Phase A » (intention → wizard pré-rempli) |

**Écart restant** : aujourd'hui l'appel IA est **monophonique** — un seul prompt, un seul appel,
un seul rôle. CrewAI apporte exactement ce qui manque : **plusieurs rôles, des tâches ordonnées,
des sorties structurées et une passe de relecture** — sans toucher au runtime.

---

## 2. Positionnement & non-objectifs

### Ce que CrewAI fait ici
- Orchestration **hors ligne** de rôles de conception : *game designer → level designer →
  scripteur KVL → équilibreur → reviewer*.
- Production d'**artefacts JSON versionnés** conformes à `KARACTI_AI_SPEC.md` §7.
- Enchaînement conditionnel (Flow) : si le reviewer rejette → boucle de correction (max 2 tours).

### Ce que CrewAI ne fait PAS (non-objectifs)
- ❌ Pas dans la boucle 60 FPS — aucun appel réseau pendant le jeu.
- ❌ Pas comme source de vérité du runtime (le framework versionné dans le dépôt l'est ; le
  **Studio AMP cloud** est à sens unique → ne pas l'utiliser comme répertoire de travail).
- ❌ Pas de réécriture de `ai_client.rs`/`ai_patch.rs` — CrewAI **alimente** ces briques,
  il ne les remplace pas.
- ❌ Pas d'écriture automatique dans `assets/` sans validation + approbation humaine.

### Décision d'architecture n°1 : sidecar Python, pont JSON
CrewAI est Python (≥ 3.10, < 3.14) → **processus séparé** dans `agents/`, communiquant par
fichiers JSON + codes de retour. Le moteur reste 100 % Rust et fonctionne **sans** le sidecar
(dégradation gracieuse : sans Python, l'IA monophonique `ai_client.rs` reste disponible).

---

## 3. Architecture cible

```
                     ┌───────────────────────────────────────────────┐
                     │  Studio Tauri (apps/studio)  —  ou moteur natif│
                     │  Champ « ✨ Décrire le niveau voulu… »         │
                     └────────────────────┬──────────────────────────┘
                                          │ Tauri command / CLI
                                          ▼
┌─ agents/ (sidecar Python, uv + CrewAI) ─────────────────────────────────┐
│  Flow « level_pipeline » :                                              │
│   ① brief_generator  → BriefSpec      (titre, genre, ton, contraintes)  │
│   ② level_designer   → LevelSpec      (schéma KARACTI_AI_SPEC §7)       │
│   ③ kvl_scripter     → KvlScriptSpec  (graphe nodes/connections)        │
│   ④ balancer         → BalanceReport  (PV, dégâts, vitesses — cibles)   │
│   ⑤ reviewer         → ReviewVerdict  (OK / rejet motivé, max 2 tours)  │
│  Modèles Pydantic = miroir exact du schéma §7 (contrat partagé)         │
│  Sortie : design/<run_id>/artefacts/*.json + journal run.json           │
└──────────────────────────────┬──────────────────────────────────────────┘
                               ▼  (JSON, contrat versionné)
┌─ Côté Rust ─────────────────────────────────────────────────────────────┐
│  Porte 1 : serde_json → LevelSpec (schéma connu)                        │
│  Porte 2 : validateur statique kvl-core (pins/typage)                   │
│  Porte 3 : recipe.rs resolver (cohérence dépendances)                   │
│  Porte 4 : sandbox de simulation §8 (100 pas, spawn non-void, refs)     │
│  → ai_patch.rs applique → scène jouable → pile undo/redo                │
│  Rejet à n'importe quelle porte : rapport d'erreur lisible côté Studio  │
└─────────────────────────────────────────────────────────────────────────┘
```

**Pourquoi 5 rôles et pas 1 gros prompt** : chaque rôle a un objectif mesurable et une sortie
typée ; le reviewer rejette motivé, ce qui réduit nettement les « niveaux jouables en apparence
mais cassés » — et rend le coût/perte auditable (`run.json`).

---

## 4. Contrat de données (le cœur du plan)

**Règle : `KARACTI_AI_SPEC.md` est la seule vérité.** Concrètement :

1. **Côté Python** : modèles Pydantic `LevelSpec`, `KvlScriptSpec`, `BriefSpec`, `BalanceReport`
   générés pour correspondre champ à champ à §7 (et au format `nodes/connections` réel de
   `assets/scripts/*.kvl.json` — ce format est plus riche que les `kvl_actions` simplifiés de
   la spec : le scripteur KVL doit produire le format réel).
2. **Côté Rust** : les structs existantes (`ai_client.rs` : `AiLevelSpec`… et les types de
   `ai_patch.rs`) sont la référence d'exécution. Ajouter le crate **`schemars`** pour *émettre*
   un JSON Schema depuis ces structs → fichier `contracts/level.schema.json` commité.
3. **Anti-dérive** : le même JSON Schema est validé des deux côtés :
   - pytest Python : chaque fixture `agents/fixtures/*.json` valide contre le schéma ;
   - `cargo test` Rust : mêmes fixtures chargées via `serde_json` + `ai_patch`.
   *Une modification du schéma casse la CI tant que les deux côtés ne sont pas mis à jour.*
4. **Versionnement** : champ `"schema_version": 1` dans chaque artefact ; rejet explicite si
   inconnu (pas de devinette silencieuse).

**Écarts connus à trancher au M0** (à vérifier dans les structs exacts) :
- La spec utilise `offset_x/offset_y` relatifs à l'ancre joueur ; `ai_patch.rs` semble déjà
  gérer les deux — figer la convention dans le contrat.
- `win_condition` : la spec dit `"DefeatBoss"` et un test utilise `"boss_defeated"` →
  énumération canonique unique (snake_case, comme `GameGenre`), aliases tolérés.

---

## 5. Structure du sidecar

```
agents/
├── pyproject.toml            # uv ; crewai (≥ 0.x récent), pydantic, jsonschema
├── uv.lock                   # verrouillé, commité
├── README.md                 # : uv sync && uv run pipeline --brief "..."
├── .env.example              # KARACTI_LLM_BASE_URL=http://127.0.0.1:1234/v1 …
├── contracts/
│   └── level.schema.json     # émis par cargo (schemars) — importé par Pydantic
├── src/karacti_crew/
│   ├── models.py             # BriefSpec / LevelSpec / KvlScriptSpec (miroir contrat)
│   ├── crews/
│   │   ├── design.py         # ①② : brief → niveau
│   │   ├── scripting.py      # ③ : niveau → graphe KVL
│   │   ├── balance.py        # ④ : niveau → rapport d'équilibrage
│   │   └── review.py         # ⑤ : reviewer + verdict structuré
│   ├── flow.py               # orchestration + boucle de correction (max 2)
│   └── provider.py           # config LLM partagée (litellm, OpenAI-compatible)
├── fixtures/                 # fichiers JSON partagés avec les tests Rust
└── runs/                     # sorties datées (gitignorées sauf runs validés)
```

**Configuration LLM** : le sidecar pointe **le même endpoint que le moteur**
(LM Studio local par défaut, OpenAI-compatible via litellm) — un seul point de config,
zéro clé à gérer en local, et le mode `OfflineLocal` reste le garde-fou sans réseau.

---

## 6. Pont côté Rust

Nouveau module `crates/engine-core/src/agents_bridge.rs` (ou binaire `cargo run --bin crew`) :

```rust
pub struct CrewRequest { pub brief: String, pub genres: Vec<GameGenre>, pub run_id: String }
pub struct CrewOutcome { pub artifacts_dir: PathBuf, pub journal: RunJournal }
// 1. spawn `uv --directory agents run flow --brief ... --out design/<run_id>/`
// 2. timeout paramétrable (défaut 300 s), stderr capturé dans le journal
// 3. si uv/Python absent → Err(NotInstalled) → le Studio retombe sur ai_client.rs monophonique
// 4. lecture des artefacts → Portes 1→4 (section 3) → ai_patch::apply
```

- Toute modification appliquée entre dans la **pile undo/redo existante** (`Ctrl+Z` réversible).
- Chaque tentative est journalisée (`design/<run_id>/run.json` : brief, prompts clés, verdicts,
  durée, fournisseur) — réutilisable comme garde-fou de coût et comme historique « IA »
  (critique design, §journal des interventions).

---

## 7. Intégration Studio Tauri (phase UI)

Panneau « ✨ Assistant » dans `apps/studio` (React 19 + xyflow déjà présents) :

1. Champ de brief + pastilles de genre (`GameGenre` existant) + bouton **Générer**.
2. Commande Tauri `run_crew(brief, genres)` → **statut streaming** (étape ①…⑤ affichée).
3. Résultat : **liste de propositions** avec aperçu (miniature de la scène, entités, win_condition).
4. Clic → **diff vue** (entités ajoutées/modifiées colorées) → `Appliquer` / `Rejeter`.
5. Rejet d'une porte de validation → message explicite dans le langage de la porte
   (« Graphe KVL invalide : pin `out_trigger` (Trigger) connecté à `score` (Int) »).

**Reprise de la critique design** : ce panneau EST la « Phase B » ; il réutilise le halo violet
`#A855F7` (déjà codé pour les patches IA) pour marquer les éléments proposés, et
Valider/Annuler = pile undo. La « Phase A » (intent bar déterministe → wizard G) reste
pertinente comme chemin sans Python et sans LLM.

---

## 8. Jalons, livrables & critères d'acceptation

| Jalon | Contenu | Critère d'acceptation (vérifiable) |
|---|---|---|
| **M0 — Fondation** (1–2 j) | `agents/` + uv + Pydantic miroir + `contracts/level.schema.json` via schemars + fixtures communes | `uv run pytest` ✅ et `cargo test ai_contract` ✅ sur les mêmes fixtures ; `crewai` atteint LM Studio (ou mock) |
| **M1 — Crew vertical** (2–3 j) | Crew `design` (①②) seul → LevelSpec → import manuel | Un brief → JSON valide → `ai_patch` → **scène jouable dans le moteur**, sans édition manuelle |
| **M2 — Pipeline complet** (3–5 j) | Flow 5 rôles + reviewer bouclé + journal `run.json` + Portes 1–4 | Brief donné → artefacts versionnés + verdict ; un graphe KVL invalide est **rejeté par la Porte 2** avec message lisible |
| **M3 — Studio** (3–5 j) | Panneau Assistant, streaming, diff, Appliquer/Rejeter, undo | Bouton Générer → proposition → Appliquer → `Ctrl+Z` annule tout |
| **M4 — MCP & durcissement** (j) | Serveur MCP Rust (`validate_quest`, `list_game_assets`, `get_design_constraints`) branché aux outils CrewAI ; CI avec LLM mocké | L'agent **s'auto-valide** via MCP avant livraison ; CI sans réseau reproduit M1–M2 |

**Dépendance** : chaque jalon est livrable indépendamment ; M1 seul vaut déjà le coup
(un crew simple > prompt monolithique).

---

## 9. Risques & atténuations

| Risque | Impact | Atténuation |
|---|---|---|
| **Dérive de schéma** Python ↔ Rust | Artefacts rejetés silencieusement | JSON Schema unique émis par `schemars`, fixtures partagées dans les deux CIs |
| **Modèle local fragile** (nemotron-3-nano-4b = 4B, les crews multi-tâches sollicitent plus) | Sorties de mauvaise qualité | Revue systématique (rôle ⑤), prompts courts + spécification §7 collée au contexte, fournisseur commutable (`.env`), fallback `recipe.rs`/`OfflineLocal` toujours dispo |
| **Latence** (5 appels × ~30 s sur LM Studio) | Mauvaise UX dans le Studio | Streaming d'étapes (M3), génération asynchrone, runs persistés |
| **Friction d'install Python** | Perte de contribution | `uv` verrouillé + README 3 commandes ; moteur fonctionne **sans** sidecar |
| **Tentation Studio AMP (export à sens unique)** | Perte de versioning | Règle : le framework dans `agents/` est la source de vérité ; Studio AMP éventuel seulement pour prototyper des crews |
| **Écriture non contrôlée dans `assets/`** | Corruption d'une scène jouable | Portes 1–4 obligatoires + approbation humaine + undo ; `design/` = zone tampon, `assets/` = écriture finale explicite |
| **Confidentialité / coûts** (fournisseur cloud) | Dépendance externe | Local-first (LM Studio) par défaut ; fournisseur cloud opt-in documenté |

---

## 10. Stratégie de tests

- **Contract tests** : fixtures JSON validées par Pydantic/jsonschema **et** par `serde_json` + `ai_patch` (M0).
- **Unitaires Rust** : garder verts les tests `ai_patch`/`recipe` existants (déjà 2 scènes de fixtures).
- **Unitaires Python** : chaque crew testé avec un **LLM mocké** (réponses de fixtures) → testable hors réseau.
- **E2E script** `scripts/crew_e2e.sh` : brief → flow mocké → Portes 1–4 → scène → `cargo test`.
- **Manuel par jalon** : checklist dans le README `agents/` (débutant : suivre les 3 commandes).

---

## 11. Ordre d'exécution recommandé

```
M0 (contrat + fixtures) ──► M1 (crew vertical jouable) ──► M2 (pipeline + portes)
                                    │                            │
                                    └──► M3 (panneau Studio) ◄────┘
                                                 │
                                                 ▼
                                        M4 (MCP + CI mockée)
```

**Le point de départ non négociable est M0** : sans contrat partagé, tout le reste accumule
de la dette de schéma. Tout le reste découle naturellement.

---

*Plan de conception uniquement — aucun code exécuté ni modifié pour ce document.*
