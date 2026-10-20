"""Crew M1 « design » : brief texte → LevelSpec JSON (contrat ``level.schema.json``).

Import paresseux de crewai (dans ``build_design_crew``) : les tests contract
tournent sans le groupe optionnel ``crew`` installé.
"""

from __future__ import annotations

from pathlib import Path

from karacti_crew.provider import LlmConfig

AGENTS_DIR = Path(__file__).resolve().parents[3]
SCHEMA_PATH = AGENTS_DIR / "contracts" / "level.schema.json"

_DESIGNER_BACKSTORY = (
    "Tu es le Lead Game Architect de KaractiRage, moteur 2D low-end first. "
    "Tu conçois des niveaux JSON strictement conformes à la spécification officielle "
    "KARACTI_AI_SPEC.md (schéma level.schema.json) : terrain, entités avec archétypes de "
    "sprites, boucle de gameplay avec conditions de victoire/défaite, actions KVL. "
    "Tu réponds UNIQUEMENT par un objet JSON valide, sans texte ni bloc de code."
)


def build_design_crew(llm: LlmConfig):
    """Construit le crew « design » (brief → GamePayload). N'appelle pas le réseau ici."""
    try:
        from crewai import Agent, Crew, Process, Task
    except ImportError as exc:  # pragma: no cover - sans groupe `crew`
        raise SystemExit(
            "Le paquet `crewai` est requis pour exécuter un crew.\n"
            "Installez-le :  cd agents && uv sync --extra crew"
        ) from exc

    schema_hints = SCHEMA_PATH.read_text(encoding="utf-8") if SCHEMA_PATH.is_file() else "{}"

    designer = Agent(
        role="Game Designer KaractiRage",
        goal="Transformer un brief joueur en niveau JSON jouable et validable par le moteur",
        backstory=_DESIGNER_BACKSTORY,
        llm=llm.litellm_model,
        verbose=False,
        allow_delegation=False,
    )

    design_task = Task(
        description=(
            "Brief de conception :\n---\n{brief}\n---\n"
            "Produis le niveau complet au format JSON. Contrat JSON Schema à respecter "
            "(champs requis : title, description ; entités : name, kind ; plateformes : width, height) :\n"
            f"{schema_hints}"
        ),
        expected_output=(
            "Un objet JSON brut du niveau (title, description, terrain, gameplay_loop, "
            "entities, kvl_actions), sans balisage markdown. La validation est faite "
            "côté hôte contre le contrat : pas de sortie structurée forcée (LM Studio "
            "refuse tool_choice objet — cf. tests live M0)."
        ),
        agent=designer,
    )

    return Crew(
        agents=[designer],
        tasks=[design_task],
        process=Process.sequential,
        verbose=False,
    )
