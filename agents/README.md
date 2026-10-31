# agents/ — Sidecar d'orchestration agentique CrewAI

Pipeline de conception (niveaux, scripts KVL, équilibrage) exécuté **à côté** du moteur Rust,
jamais dans la boucle 60 FPS. Plan complet : [`../PLAN_CONCEPTION_CREWAI.md`](../PLAN_CONCEPTION_CREWAI.md).

## Installation

```bash
cd agents
uv sync                # pydantic + jsonschema + pytest (léger, suffisant pour les tests)
uv sync --extra crew   # + crewai, pour exécuter réellement un crew (M1+)
```

Python ≥ 3.10 (uv fournit l'interpréteur automatiquement).

## Tests (contrat partagé Rust ⇄ Python)

```bash
cd agents && uv run pytest          # 8 tests : contrat, aliases, config LLM
```

Côté Rust, les MÊMES fixtures sont testées (désérialisation + sandbox §8) :

```bash
cargo test -p engine-core contract_
```

**Le contrat** `contracts/level.schema.json` est émis par le Rust (source de vérité) :

```bash
UPDATE_CONTRACTS=1 cargo test -p engine-core contract_schema_is_current
```

Toute modification des structs `AiGenerated*` casse `pytest` tant que le contrat n'est pas
régénéré, et réciproquement — c'est voulu (anti-dérive, §4 du plan).

## CLI

```bash
# Valider un JSON contre le contrat (sans LLM, sans crewai) :
uv run python -m karacti_crew.flow --validate fixtures/level_dungeon_basics.json

# Générer un niveau depuis un brief (M1 : requiert --extra crew + LLM local) :
uv run python -m karacti_crew.flow --brief "Donjon de plateformes avec un boss" --out runs/
```

## Configuration LLM

Le sidecar lit **le même** `karactirage.ai.json` que le moteur (LM Studio local par défaut).
Variables d'environnement possibles (voir `.env.example`) : `KARACTI_LLM_API_KEY` si un
fournisseur cloud est choisi. Local-first : aucun appel réseau requis pour les tests.

## Arborescence

```
agents/
├── contracts/level.schema.json   # contrat émis par schemars (Rust) — ne pas éditer à la main
├── fixtures/*.json               # fixtures partagées des deux CIs
├── src/karacti_crew/
│   ├── models.py                 # miroirs Pydantic des structs AiGenerated*
│   ├── provider.py               # config LLM partagée avec ai_client.rs
│   ├── crews/design.py           # crew M1 : brief → GamePayload (import crewai paresseux)
│   └── flow.py                   # CLI : --validate / --brief
└── tests/                        # pytest : contract + provider (hors réseau)
```

## État

Jalon **M0** livré (contrat + fixtures + tests verts des deux côtés).
M1 (crew exécuté de bout en bout), M2 (pipeline 5 rôles), M3 (panneau Studio) et
M4 (MCP) sont décrits dans le plan — non implémentés ici.
