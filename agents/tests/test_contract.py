"""Tests de contrat partagés Rust ⇄ Python (PLAN_CONCEPTION_CREWAI.md, jalon M0).

Le JSON Schema ``agents/contracts/level.schema.json`` est émis par le Rust
(``UPDATE_CONTRACTS=1 cargo test -p engine-core contract_schema_is_current``)
et fait foi. Ces tests vérifient que le miroir Pydantic n'a pas dérivé :
* mêmes noms de champs,
* mêmes champs requis,
* fixtures valides des deux côtés (les mêmes fichiers sont aussi testés par
  ``contract_fixtures_pass_engine_and_sandbox`` côté cargo).
"""

from __future__ import annotations

import json
from pathlib import Path

import jsonschema
import pytest

from karacti_crew.models import (
    Entity,
    GameplayLoop,
    GamePayload,
    Hazard,
    KvlAction,
    Platform,
    Terrain,
)

AGENTS_DIR = Path(__file__).resolve().parents[1]
SCHEMA_PATH = AGENTS_DIR / "contracts" / "level.schema.json"
FIXTURES_DIR = AGENTS_DIR / "fixtures"

# $defs du schéma Rust → modèle miroir
DEF_MODELS = {
    "AiGeneratedEntity": Entity,
    "AiGeneratedGameplayLoop": GameplayLoop,
    "AiGeneratedHazard": Hazard,
    "AiGeneratedKvlAction": KvlAction,
    "AiGeneratedPlatform": Platform,
    "AiGeneratedTerrain": Terrain,
}


@pytest.fixture(scope="module")
def schema() -> dict:
    assert SCHEMA_PATH.is_file(), (
        f"Contrat absent : {SCHEMA_PATH} — régénérez avec "
        "UPDATE_CONTRACTS=1 cargo test -p engine-core contract_schema_is_current"
    )
    return json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))


@pytest.fixture(scope="module")
def fixtures() -> list[dict]:
    paths = sorted(FIXTURES_DIR.glob("*.json"))
    assert len(paths) >= 2, "Au moins 2 fixtures partagées attendues"
    return [json.loads(p.read_text(encoding="utf-8")) for p in paths]


def test_schema_is_draft_2020(schema: dict) -> None:
    assert schema.get("$schema", "").endswith("/draft/2020-12/schema")
    assert set(schema["properties"]) == {
        "title", "description", "primary_genre", "terrain",
        "gameplay_loop", "entities", "kvl_actions", "variables",
    }


def test_fixtures_validate_against_rust_schema(schema: dict, fixtures: list[dict]) -> None:
    for data in fixtures:
        jsonschema.validate(instance=data, schema=schema)


def test_fixtures_parse_into_pydantic(fixtures: list[dict]) -> None:
    for data in fixtures:
        payload = GamePayload.from_fixture(data)
        assert payload.title
        assert payload.description


def test_field_names_match_schema(schema: dict) -> None:
    # Racine : GamePayload
    assert set(GamePayload.model_fields) == set(schema["properties"]), (
        "Dérive de champs entre GamePayload et le contrat Rust"
    )
    # Sous-objets : chaque $def
    for def_name, model in DEF_MODELS.items():
        def_schema = schema["$defs"][def_name]
        assert set(model.model_fields) == set(def_schema["properties"]), (
            f"Dérive de champs entre {model.__name__} et $defs.{def_name}"
        )


def test_required_fields_match_schema(schema: dict) -> None:
    root_required = set(schema.get("required", []))
    pydantic_required = {n for n, f in GamePayload.model_fields.items() if f.is_required()}
    assert pydantic_required == root_required, (
        f"Champs requis divergents (racine) : {pydantic_required} != {root_required}"
    )
    for def_name, model in DEF_MODELS.items():
        def_schema = schema["$defs"][def_name]
        expected = set(def_schema.get("required", []))
        actual = {n for n, f in model.model_fields.items() if f.is_required()}
        assert actual == expected, (
            f"Champs requis divergents pour {model.__name__} : {actual} != {expected}"
        )


def test_roundtrip_revalidates_against_schema(schema: dict, fixtures: list[dict]) -> None:
    """Un payload validé puis resérialisé par Pydantic reste conforme au contrat."""
    for data in fixtures:
        dumped = GamePayload.from_fixture(data).model_dump(mode="json")
        jsonschema.validate(instance=dumped, schema=schema)


def test_alias_inputs_are_accepted() -> None:
    """Les alias Rust (x/y/kind) acceptés par serde restent acceptés côté Python."""
    data = {
        "title": "Alias",
        "description": "Champs x/y/kind",
        "terrain": {"platforms": [{"width": 100.0, "height": 20.0, "x": -50.0, "y": 10.0}]},
        "entities": [{"name": "P", "kind": "player", "x": 1.0, "y": 2.0}],
    }
    payload = GamePayload.from_fixture(data)
    assert payload.terrain.platforms[0].offset_x == -50.0
    assert payload.entities[0].offset_y == 2.0
