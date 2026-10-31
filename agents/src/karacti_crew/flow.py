"""Pipeline de conception — CLI du sidecar (validation M0, exécution M1).

Usage :
    # Valider un fichier contre le contrat (sans LLM, sans crewai) :
    uv run python -m karacti_crew.flow --validate fixtures/level_dungeon_basics.json

    # Générer un niveau à partir d'un brief (requiert `uv sync --extra crew` + LLM local) :
    uv run python -m karacti_crew.flow --brief "Donjon de plateformes avec un boss" --out runs/
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
from pathlib import Path

import jsonschema

from karacti_crew.models import GamePayload
from karacti_crew.provider import LlmConfig, load_llm_config

AGENTS_DIR = Path(__file__).resolve().parents[2]  # agents/src/karacti_crew/flow.py → agents/
DEFAULT_SCHEMA = AGENTS_DIR / "contracts" / "level.schema.json"


def load_schema(path: Path) -> dict:
    if not path.is_file():
        sys.exit(
            f"Contrat absent : {path}\n"
            "Régénérez-le depuis le Rust :\n"
            "  UPDATE_CONTRACTS=1 cargo test -p engine-core contract_schema_is_current"
        )
    return json.loads(path.read_text(encoding="utf-8"))


def validate_payload(data: dict, schema: dict) -> GamePayload:
    """Porte 1 (schéma) + porte miroir Pydantic. Lève sur échec."""
    jsonschema.validate(instance=data, schema=schema)
    return GamePayload.model_validate(data)


def extract_json_object(text: str) -> dict:
    """Extrait l'objet JSON d'une réponse LLM en texte libre.

    Les petits modèles enveloppent parfois leur JSON dans un bloc ```json```
    ou l'entourent de texte. La sortie structurée forcée (tool_choice objet)
    est indisponible sur LM Studio : on parse nous-mêmes, la validation
    restant faite par ``validate_payload`` (aucune porte contournée).
    """
    raw = text.strip()
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        pass
    fenced = re.search(r"```(?:json)?\s*(\{.*\})\s*```", raw, re.DOTALL)
    candidates = [fenced.group(1)] if fenced else []
    start, end = raw.find("{"), raw.rfind("}")
    if start != -1 and end > start:
        candidates.append(raw[start : end + 1])
    for cand in candidates:
        try:
            return json.loads(cand)
        except json.JSONDecodeError:
            continue
    raise ValueError(
        "Aucun objet JSON exploitable dans la réponse du LLM. "
        f"Début de réponse : {raw[:200]!r}"
    )


def cmd_validate(args: argparse.Namespace) -> int:
    schema = load_schema(Path(args.schema))
    fixture = Path(args.validate)
    data = json.loads(fixture.read_text(encoding="utf-8"))
    payload = validate_payload(data, schema)
    print(f"OK  {fixture.name} : valide contre le contrat + miroir Pydantic "
          f"({len(payload.entities)} entités, {len(payload.kvl_actions)} actions KVL)")
    return 0


def cmd_generate(args: argparse.Namespace) -> int:
    schema = load_schema(Path(args.schema))
    llm: LlmConfig = load_llm_config(AGENTS_DIR.parent)

    # CrewAI/litellm lit la racine OpenAI-compatible depuis l'environnement.
    import os

    os.environ.setdefault("OPENAI_API_BASE", llm.base_url)
    os.environ.setdefault("OPENAI_API_KEY", os.environ.get("KARACTI_LLM_API_KEY", "local"))

    from karacti_crew.crews.design import build_design_crew

    crew = build_design_crew(llm)
    print(f"[crew] brief → {llm.provider} · {llm.model} (timeout {llm.timeout_seconds}s) …")
    started = time.time()
    result = crew.kickoff(inputs={"brief": args.brief})
    elapsed = time.time() - started

    raw = result.pydantic.model_dump() if getattr(result, "pydantic", None) else str(result)
    try:
        data = raw if isinstance(raw, dict) else extract_json_object(raw)
        payload = validate_payload(data, schema)  # Porte 1+miroir avant tout écriture
    except (ValueError, json.JSONDecodeError, jsonschema.ValidationError) as exc:
        print(f"ÉCHEC — payload refusé par les portes : {exc}")
        return 1

    out_dir = Path(args.out)
    run_id = time.strftime("%Y%m%d-%H%M%S")
    run_dir = out_dir / run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    (run_dir / "level.json").write_text(
        json.dumps(payload.model_dump(mode="json"), ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    (run_dir / "run.json").write_text(
        json.dumps(
            {
                "brief": args.brief,
                "provider": llm.provider,
                "model": llm.model,
                "elapsed_seconds": round(elapsed, 1),
                "schema_validated": True,
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    print(f"OK  {run_dir}/level.json ({elapsed:.1f}s) — portes Rust restantes : "
          "kvl-core, recipe, sandbox §8 puis import ai_patch.")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="karacti_crew", description=__doc__)
    parser.add_argument("--validate", metavar="FICHIER",
                        help="Valider un JSON contre le contrat, sans LLM")
    parser.add_argument("--brief", metavar="TEXTE",
                        help="Générer un niveau à partir d'un brief (requiert crewai + LLM)")
    parser.add_argument("--out", default="runs", help="Répertoire des runs (défaut : runs/)")
    parser.add_argument("--schema", default=str(DEFAULT_SCHEMA),
                        help="Chemin du JSON Schema partagé")
    args = parser.parse_args(argv)

    if args.validate:
        return cmd_validate(args)
    if args.brief:
        return cmd_generate(args)
    parser.error("Indiquez --validate FICHIER ou --brief TEXTE")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
