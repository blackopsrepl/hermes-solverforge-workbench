"""SolverForge Workbench for Hermes: optimization models you configure and solve.

Stock models ship in one attested SolverForge engine binary. This package
keeps models, datasets, profiles and runs, exposes them to the agent, runs
solves in a detached worker, and ships the Desktop surfaces in ``desktop/``.
"""

from __future__ import annotations

import os
import threading
from pathlib import Path

from .commands import cli_handler, cli_setup, slash_handler
from .engine_bin import StockEngine
from .service import Workbench
from .store import Store
from .tools import tool_table

NAME = "solverforge-workbench"
TOOLSET = "solverforge-workbench"
# Tools that delete the user's data. Each needs a human yes, every time.
GATED = {"solverforge_workbench_dataset_delete", "solverforge_workbench_profile_delete"}

_lock = threading.Lock()
# One workbench per plugin data directory. The data directory follows the Hermes
# home of the profile being served, which one process can switch per request.
_workbenches: dict[Path, Workbench] = {}


def data_dir() -> Path:
    from plugins.plugin_storage import plugin_data_dir

    return Path(plugin_data_dir(NAME))


def broadcast(what: str, detail: dict) -> None:
    """Push a change to the Desktop half; a no-op where no Desktop is attached."""
    from hermes_cli.plugin_events import broadcast_plugin_event

    broadcast_plugin_event(NAME, "changed", {"what": what, **detail})


def workbench(get_config=None) -> Workbench:
    """The workbench for the profile being served. The Desktop backend shares it with the tools."""
    root = data_dir()
    with _lock:
        found = _workbenches.get(root)
        if found is None:
            config = get_config or (lambda key, default=None: default)

            def parallel() -> int:
                value = config("parallel_solves", 0) or 0
                return int(value) if int(value) > 0 else max(1, (os.cpu_count() or 2) // 4)

            found = _workbenches[root] = Workbench(
                Store.at(root / "workbench.db"),
                root,
                lambda: StockEngine(root, override=str(config("binary", "") or "")),
                parallel_solves=parallel,
                on_change=broadcast,
            )
        return found


def approval_text(wb: Workbench, tool_name: str, args: dict) -> str:
    if tool_name == "solverforge_workbench_dataset_delete":
        dataset = wb.store.dataset(args.get("dataset", "")) or {}
        runs = len(wb.store.runs(dataset_id=args.get("dataset", "")))
        return f"Delete dataset “{dataset.get('name', args.get('dataset'))}” and its {runs} run(s)?"
    profile = wb.store.profile(args.get("profile", "")) or wb.store.profile_named(
        args.get("model", ""), args.get("profile", "")) or {}
    return f"Delete profile “{profile.get('name', args.get('profile'))}” of {args.get('model')}?"


def register(ctx) -> None:
    def wb() -> Workbench:
        return workbench(ctx.get_config)

    for schema, handler in tool_table(wb):
        ctx.register_tool(name=schema["name"], toolset=TOOLSET, schema=schema, handler=handler, emoji="⚙")

    def gate(tool_name: str = "", args: dict | None = None, **_kwargs):
        if tool_name not in GATED:
            return None
        args = args or {}
        # Scope "always allow" to this exact record, so one approval never covers a later deletion.
        target = args.get("dataset") or args.get("profile") or ""
        return {"action": "approve", "message": approval_text(wb(), tool_name, args),
                "rule_key": f"{tool_name}:{target}"}

    ctx.register_hook("pre_tool_call", gate)

    ctx.register_command(
        "solverforge-workbench",
        lambda raw="", **kw: slash_handler(wb(), raw),
        description="SolverForge Workbench: status, install, models, runs",
        args_hint="[status|install|models|runs]",
    )
    ctx.register_cli_command(
        name=NAME,
        help="Install and check the SolverForge Workbench engine",
        setup_fn=cli_setup,
        handler_fn=lambda args: cli_handler(wb(), args),
    )
    ctx.register_skill("solverforge-workbench", Path(__file__).parent / "skills" / "solverforge-workbench" / "SKILL.md")
