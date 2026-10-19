"""Tests de résolution de la config LLM partagée — aucun réseau, aucun LLM."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from karacti_crew.provider import load_llm_config

AGENTS_DIR = Path(__file__).resolve().parents[1]
REPO_ROOT = AGENTS_DIR.parent


def test_engine_config_file_is_used() -> None:
    """Le sidecar lit le MÊME karactirage.ai.json que le moteur Rust."""
    engine_cfg = json.loads((REPO_ROOT / "karactirage.ai.json").read_text(encoding="utf-8"))
    llm = load_llm_config(REPO_ROOT)
    assert llm.provider == engine_cfg["provider"]
    assert llm.model == engine_cfg["custom_model"]
    assert llm.chat_completions_url.endswith("/chat/completions")
    assert llm.base_url.startswith("http")
    assert llm.litellm_model.startswith("openai/")


def test_defaults_without_config_file(tmp_path: Path) -> None:
    llm = load_llm_config(tmp_path)  # aucun fichier → défaut miroir Rust (Ollama)
    assert llm.provider == "Ollama"
    # Racine OpenAI-compatible (avec /v1) : /chat/completions est ré-ajouté par chat_completions_url
    assert llm.base_url == "http://localhost:11434/v1"
    assert llm.model == "qwen2.5-coder"
    assert llm.chat_completions_url == "http://localhost:11434/v1/chat/completions"


def test_unknown_provider_without_endpoint_is_rejected(tmp_path: Path) -> None:
    (tmp_path / "karactirage.ai.json").write_text(
        json.dumps({"provider": "Inconnu", "timeout_seconds": 5}), encoding="utf-8"
    )
    with pytest.raises(ValueError, match="Fournisseur inconnu"):
        load_llm_config(tmp_path)
