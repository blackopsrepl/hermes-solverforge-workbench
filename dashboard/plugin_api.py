"""Backend for the Desktop half: /api/plugins/solverforge-workbench/*.

Reads go through the same tool handlers the agent uses, so the page and the
chat see one store and one set of refusals. Every write is reachable only from
a button the user clicks; that click is the decision. A deletion also needs
`confirm: true` in its body, which only the page's confirm dialog sends, and
that dialog shows the same sentence the agent's approval prompt shows.
"""

from __future__ import annotations

import importlib
import importlib.util
import json
import sys
import threading
from pathlib import Path
from typing import Any

from fastapi import APIRouter, Body
from fastapi.responses import JSONResponse

_ROOT = Path(__file__).resolve().parents[1]
_PACKAGE = "hermes_solverforge_workbench_dashboard_pkg"
_LOCK = threading.Lock()
_LOADED: list = []

# Tool names the page may call, by route action.
READS = {
    "status": "solverforge_workbench_status",
    "models": "solverforge_workbench_models",
    "model": "solverforge_workbench_model",
    "datasets": "solverforge_workbench_datasets",
    "dataset": "solverforge_workbench_dataset",
    "runs": "solverforge_workbench_runs",
    "run": "solverforge_workbench_run",
    "compare": "solverforge_workbench_compare",
    "fill": "solverforge_workbench_fill",
    "experiments": "solverforge_workbench_experiments",
    "experiment": "solverforge_workbench_experiment",
    "trends": "solverforge_workbench_trends",
    "model_build": "solverforge_workbench_model_build",
    "edits": "solverforge_workbench_edits",
    "changes": "solverforge_workbench_changes",
}
WRITES = {
    "solve": "solverforge_workbench_solve",
    "evaluate": "solverforge_workbench_evaluate",
    "cancel": "solverforge_workbench_cancel",
    "run_update": "solverforge_workbench_run_update",
    "profile_save": "solverforge_workbench_profile_save",
    "profile_default": "solverforge_workbench_profile_default",
    "dataset_create": "solverforge_workbench_dataset_create",
    "fill_apply": "solverforge_workbench_fill_apply",
    "experiment_create": "solverforge_workbench_experiment_create",
    "experiment_cancel": "solverforge_workbench_experiment_cancel",
    "dataset_update": "solverforge_workbench_dataset_update",
    "dataset_patch": "solverforge_workbench_dataset_patch",
    "dataset_delete": "solverforge_workbench_dataset_delete",
    "export": "solverforge_workbench_export",
    "profile_delete": "solverforge_workbench_profile_delete",
    "profile_export": "solverforge_workbench_profile_export",
    "profile_import": "solverforge_workbench_profile_import",
    "experiment_delete": "solverforge_workbench_experiment_delete",
    "model_remove": "solverforge_workbench_model_remove",
    "model_register": "solverforge_workbench_model_register",
    # Stores nothing, but carries a whole plan: a POST like the writes.
    "dataset_check": "solverforge_workbench_dataset_check",
}
# Writes that delete: refused unless the body carries confirm: true.
CONFIRMED = {"dataset_delete", "profile_delete", "experiment_delete", "model_remove"}
STATUS = {"unknown_model": 404, "unknown_dataset": 404, "unknown_run": 404, "unknown_profile": 404,
          "unknown_experiment": 404,
          "no_view": 404, "stale": 409, "busy": 409, "engine_missing": 503, "storage_error": 503}


def _package():
    # The dashboard imports this file as a lone module; load the plugin package beside it once.
    # Requests run on a thread pool, so the first load is serialized and a half-built package is
    # never visible to a second request.
    if _LOADED:
        return _LOADED[0]
    with _LOCK:
        if not _LOADED:
            spec = importlib.util.spec_from_file_location(
                _PACKAGE, _ROOT / "__init__.py", submodule_search_locations=[str(_ROOT)])
            assert spec is not None and spec.loader is not None
            module = importlib.util.module_from_spec(spec)
            sys.modules[_PACKAGE] = module
            try:
                spec.loader.exec_module(module)
            except BaseException:
                sys.modules.pop(_PACKAGE, None)
                raise
            _LOADED.append(module)
            # This process serves the Desktop: follow live solves and builds here and push them to it.
            module.watcher.start()
    return _LOADED[0]


def _config(key: str, default=None):
    from hermes_cli.config import load_config_readonly

    entries = (load_config_readonly().get("plugins") or {}).get("entries") or {}
    return ((entries.get("solverforge-workbench") or {}).get("settings") or {}).get(key, default)


def _call(tool: str, args: dict[str, Any]) -> JSONResponse:
    package = _package()
    tools = dict((schema["name"], handler)
                 for schema, handler in package.tool_table(lambda: package.workbench(_config)))
    result = json.loads(tools[tool](args))
    code = 200 if result.get("ok") else STATUS.get(result.get("code", ""), 400)
    return JSONResponse(result, status_code=code)


router = APIRouter()


@router.get("/view")
def view(run: str = "", dataset: str = "") -> JSONResponse:
    """The plan of a run or dataset, drawn by its model, as a self-contained page for the sandboxed frame."""
    package = _package()
    service = importlib.import_module(f"{package.__name__}.service")
    views = importlib.import_module(f"{package.__name__}.views")
    try:
        drawn = package.workbench(_config).view(run_id=run or None, dataset_id=dataset or None)
    except service.WorkbenchError as exc:
        return JSONResponse({"ok": False, "code": exc.code, "error": exc.message},
                            status_code=STATUS.get(exc.code, 400))
    return JSONResponse({"ok": True, **views.drawn(drawn)})


@router.get("/confirm")
def confirm(action: str, model: str = "", dataset: str = "", profile: str = "", experiment: str = "") -> JSONResponse:
    """The sentence a deletion asks before it happens, the same one the agent's approval prompt shows."""
    if action not in CONFIRMED:
        return JSONResponse({"ok": False, "code": "unknown_action", "error": f"{action!r} asks nothing"}, status_code=404)
    package = _package()
    args = {k: v for k, v in {"model": model, "dataset": dataset, "profile": profile, "experiment": experiment}.items() if v}
    return JSONResponse({"ok": True, "text": package.approval_text(package.workbench(_config), WRITES[action], args)})


@router.get("/{action}")
def read(action: str, model: str = "", dataset: str = "", run: str = "", runs: str = "", experiment: str = "",
         items: str = "", top: int = 5, solve_profile: str = "", include_plan: bool = False,
         max_matches: int = 10, build: str = "") -> JSONResponse:
    # `profile` is the host's own parameter: the Desktop adds the Hermes profile it routes to on
    # every request, so the workbench profile a read is scored with travels as `solve_profile`.
    if action not in READS:
        return JSONResponse({"ok": False, "code": "unknown_action", "error": f"no read named {action!r}"},
                            status_code=404)
    args: dict[str, Any] = {"model": model or None, "dataset": dataset or None, "run": run or None}
    if action == "run":
        args.update(include_plan=include_plan, max_matches=max(0, min(max_matches, 50)))
    if action == "dataset":
        args["include_plan"] = include_plan
    if action == "compare":
        args = {"runs": [r for r in runs.split(",") if r]}
    if action == "experiment":
        args = {"experiment": experiment}
    if action == "model_build":
        args = {"build": build or None, "model": model or None}
    if action == "fill":
        args = {"run": run or None, "dataset": dataset or None, "top": max(1, min(top, 50)),
                "profile": solve_profile or None, "items": [i for i in items.split(",") if i] or None}
    return _call(READS[action], {k: v for k, v in args.items() if v is not None})


@router.post("/actions/{action}")
def write(action: str, body: Any = Body(default=None)) -> JSONResponse:
    if action not in WRITES:
        return JSONResponse({"ok": False, "code": "unknown_action", "error": f"no action named {action!r}"},
                            status_code=404)
    if not isinstance(body, dict):
        return JSONResponse({"ok": False, "code": "invalid_body", "error": "send a JSON object"}, status_code=400)
    body = dict(body)
    confirmed = body.pop("confirm", None) is True
    if action in CONFIRMED and not confirmed:
        return JSONResponse({"ok": False, "code": "confirm_required", "error": "a deletion needs confirm: true"},
                            status_code=400)
    return _call(WRITES[action], body)
