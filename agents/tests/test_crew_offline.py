"""Smoke test hors ligne du crew M1 — construit le crew SANS kickoff (aucun appel réseau).

Saute proprement si le groupe optionnel ``crew`` (crewai) n'est pas installé :
```uv sync --extra crew``` pour l'activer.
"""

from __future__ import annotations

import pytest

crewai = pytest.importorskip("crewai", reason="groupe `crew` non installé (uv sync --extra crew)")

from karacti_crew.crews.design import build_design_crew  # noqa: E402
from karacti_crew.provider import LlmConfig  # noqa: E402


def test_design_crew_builds_without_network() -> None:
    llm = LlmConfig(
        provider="LmStudio",
        base_url="http://127.0.0.1:1234/v1",
        model="nvidia/nemotron-3-nano-4b",
        timeout_seconds=5,
    )
    crew = build_design_crew(llm)
    assert len(crew.agents) == 1
    assert len(crew.tasks) == 1
    assert crew.agents[0].role == "Game Designer KaractiRage"
