"""The /solverforge-workbench slash command and the `hermes solverforge-workbench` CLI command."""

from __future__ import annotations

import json

from .engine_bin import ENGINE_VERSION, EngineError, target_triple
from .service import Workbench, WorkbenchError

USAGE = (
    "/solverforge-workbench status    engine, models and the solve queue\n"
    "/solverforge-workbench install   download the SolverForge engine (verified by checksum)\n"
    "/solverforge-workbench models    list the models\n"
    "/solverforge-workbench runs      list recent runs"
)


def slash_handler(wb: Workbench, raw: str) -> str:
    words = (raw or "").split()
    verb = words[0].lower() if words else "status"
    try:
        if verb == "status":
            return json.dumps(wb.status(), indent=2)
        if verb == "install":
            # Typing the command is the user's own decision to download the engine.
            messages: list[str] = []
            wb.engine.install(log=messages.append)
            return "\n".join(messages)
        if verb == "models":
            models = wb.models()
            if not models:
                return "No models yet: install the engine with /solverforge-workbench install."
            return "\n".join(f"{m['name']} ({m['id']}): {m['constraints']} constraints, "
                             f"{m['datasets']} dataset(s)" for m in models)
        if verb == "runs":
            runs = wb.runs()[:20]
            if not runs:
                return "No runs yet."
            return "\n".join(f"{r['id']}  {r['model_id']}  {r['status']}  {r['score'] or ''}" for r in runs)
    except (WorkbenchError, EngineError) as exc:
        return f"{exc.code}: {exc.message}"
    return USAGE


def cli_setup(parser) -> None:
    sub = parser.add_subparsers(dest="action")
    sub.add_parser("status", help="Show the engine and the solve queue")
    sub.add_parser("install", help=f"Download engine {ENGINE_VERSION} for this platform, verified by checksum")


def cli_handler(wb: Workbench, args) -> int:
    action = getattr(args, "action", None) or "status"
    try:
        if action == "install":
            wb.engine.install()
            return 0
        status = wb.status()
        status["platform"] = target_triple()
        print(json.dumps(status, indent=2))
        return 0
    except (WorkbenchError, EngineError) as exc:
        print(f"{exc.code}: {exc.message}")
        return 1
