"""Résolution de la configuration LLM partagée avec le moteur Rust.

Le moteur lit ``karactirage.ai.json`` (cf. ``AiClientConfig::load_or_default``).
Le sidecar lit le **même** fichier → un seul point de vérité pour endpoint/modèle,
que ce soit le moteur ou les crews qui appellent le LLM.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

# Tables miroirs de `AiProvider::default_endpoint` / `default_model` (ai_client.rs).
_DEFAULT_ENDPOINTS = {
    "Ollama": "http://localhost:11434/v1/chat/completions",
    "LmStudio": "http://localhost:1234/v1/chat/completions",
    "Claude": "https://api.anthropic.com/v1/messages",
    "Gemini": "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
    "DeepSeek": "https://api.deepseek.com/v1/chat/completions",
    "OpenAI": "https://api.openai.com/v1/chat/completions",
}
_DEFAULT_MODELS = {
    "Ollama": "qwen2.5-coder",
    "LmStudio": "local-model",
    "Claude": "claude-3-5-sonnet-20241022",
    "Gemini": "gemini-1.5-flash",
    "DeepSeek": "deepseek-chat",
    "OpenAI": "gpt-4o-mini",
}


@dataclass(frozen=True)
class LlmConfig:
    provider: str
    base_url: str  # racine OpenAI-compatible (sans /chat/completions)
    model: str
    timeout_seconds: int

    @property
    def chat_completions_url(self) -> str:
        if self.base_url.endswith("/chat/completions"):
            return self.base_url
        return self.base_url.rstrip("/") + "/chat/completions"

    @property
    def litellm_model(self) -> str:
        """Format attendu par CrewAI/litellm pour une API compatible OpenAI locale."""
        if self.model.startswith(("openai/", "anthropic/", "gemini/")):
            return self.model
        return f"openai/{self.model}"


def load_llm_config(repo_root: Path | str) -> LlmConfig:
    """Charge ``karactirage.ai.json`` depuis la racine du dépôt (fallback : Ollama local)."""
    root = Path(repo_root)
    path = root / "karactirage.ai.json"
    raw: dict = {}
    if path.is_file():
        try:
            raw = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            raw = {}

    provider = raw.get("provider", "Ollama")
    endpoint = raw.get("custom_endpoint") or _DEFAULT_ENDPOINTS.get(provider)
    if not endpoint:
        raise ValueError(f"Fournisseur inconnu sans endpoint custom : {provider!r}")
    model = raw.get("custom_model") or _DEFAULT_MODELS.get(provider, "local-model")
    timeout = int(raw.get("timeout_seconds", 120))

    # Racine OpenAI-compatible : on retire le suffixe /chat/completions pour litellm.
    base_url = endpoint
    if base_url.endswith("/chat/completions"):
        base_url = base_url[: -len("/chat/completions")]

    return LlmConfig(
        provider=provider,
        base_url=base_url,
        model=model,
        timeout_seconds=timeout,
    )
