"""Modèles Pydantic miroirs des structs Rust ``AiGenerated*`` (ai_client.rs).

Contraintes du miroir (testées par ``tests/test_contract.py``) :
* **Noms de champs** identiques au JSON Schema émis par ``schemars`` ;
* **Champs requis** identiques (une struct Rust sans ``#[serde(default)]`` est requise ici) ;
* Champs ``Option<String>``/``Option<T>`` → ``None`` par défaut, typage nullable accepté ;
* ``extra="ignore"`` comme serde (champs inconnus ignorés, non refusés) ;
* Les alias de désérialisation Rust (``x``/``y``/``kind``) sont reproduits côté entrée.

⚠️ Le JSON Schema (émis par le Rust) fait foi : en cas de divergence, c'est ce miroir
qui doit être corrigé puis le contrat régénéré par ``UPDATE_CONTRACTS=1``.
"""

from __future__ import annotations

from typing import Any, Optional

from pydantic import AliasChoices, BaseModel, ConfigDict, Field

# Config commune : tolérance aux champs inconnus (miroir serde) + aliases populables.
_MIRROR = ConfigDict(populate_by_name=True, extra="ignore")


class Platform(BaseModel):
    """Miroir ``AiGeneratedPlatform`` — requis : width, height."""

    model_config = _MIRROR

    name: str = "Plateforme"
    offset_x: float = Field(0.0, validation_alias=AliasChoices("offset_x", "x"))
    offset_y: float = Field(0.0, validation_alias=AliasChoices("offset_y", "y"))
    width: float
    height: float
    is_solid: bool = True
    color_hex: Optional[str] = None


class Hazard(BaseModel):
    """Miroir ``AiGeneratedHazard`` — requis : width, height."""

    model_config = _MIRROR

    name: str = "Piege_Danger"
    offset_x: float = Field(0.0, validation_alias=AliasChoices("offset_x", "x"))
    offset_y: float = Field(0.0, validation_alias=AliasChoices("offset_y", "y"))
    width: float
    height: float
    damage: float = 35.0
    hazard_type: Optional[str] = Field(None, validation_alias=AliasChoices("hazard_type", "kind"))


class Terrain(BaseModel):
    """Miroir ``AiGeneratedTerrain`` — aucun champ requis (tout a un défaut)."""

    model_config = _MIRROR

    theme: str = "dungeon"
    platforms: list[Platform] = Field(default_factory=list)
    hazards: list[Hazard] = Field(default_factory=list)


class GameplayLoop(BaseModel):
    """Miroir ``AiGeneratedGameplayLoop`` — aucun champ requis."""

    model_config = _MIRROR

    objective: str = "Explorer et triompher de l'épreuve"
    win_condition: str = "CollectAllKeysThenExit"
    lose_condition: str = "PlayerHealthZero"
    target_item_name: Optional[str] = None
    win_fanfare: Optional[str] = None


class Entity(BaseModel):
    """Miroir ``AiGeneratedEntity`` — requis : name, kind."""

    model_config = _MIRROR

    name: str
    kind: str
    offset_x: float = Field(0.0, validation_alias=AliasChoices("offset_x", "x"))
    offset_y: float = Field(0.0, validation_alias=AliasChoices("offset_y", "y"))
    width: float = 32.0
    height: float = 32.0
    health: float = 3.0
    color_hex: Optional[str] = None
    sprite_archetype: Optional[str] = None
    sprite_ascii: Optional[list[str]] = None
    sprite_palette: Optional[dict[str, str]] = None
    movement: Optional[str] = None
    has_collider: bool = False
    has_vision: bool = False
    is_flocking: bool = False


class KvlAction(BaseModel):
    """Miroir ``AiGeneratedKvlAction`` — requis : event, action."""

    model_config = _MIRROR

    event: str
    action: str
    target_entity: Optional[str] = None
    condition: Optional[str] = None
    on_true_action: Optional[str] = None
    on_false_action: Optional[str] = None
    second_action: Optional[str] = None


class GamePayload(BaseModel):
    """Miroir ``AiGeneratedGamePayload`` — requis : title, description.

    C'est la sortie attendue d'un crew de conception (spec KARACTI_AI_SPEC.md §7)
    et l'entrée de ``AiPatchBuilder::build_from_generated_payload`` côté moteur.
    """

    model_config = _MIRROR

    title: str
    description: str
    primary_genre: Optional[str] = None
    terrain: Optional[Terrain] = None
    gameplay_loop: Optional[GameplayLoop] = None
    entities: list[Entity] = Field(default_factory=list)
    kvl_actions: list[KvlAction] = Field(default_factory=list)
    variables: Optional[dict[str, str]] = None

    # ---- Aides de validation (contrat, pas de logique moteur ici) ----

    @classmethod
    def from_fixture(cls, data: dict[str, Any]) -> "GamePayload":
        """Valide un dict (fichier fixture / sortie crew) contre le miroir Pydantic."""
        return cls.model_validate(data)
