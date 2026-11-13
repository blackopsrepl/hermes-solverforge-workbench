"""Backend for the Desktop half: /api/plugins/solverforge-workbench/*.

Reads go through the same tool handlers the agent uses, so the page and the
chat see one store and one set of refusals. The writes (solve, evaluate,
cancel, save a profile, make a profile the default) are reachable only from a
button the user clicks; that click is the decision. Nothing here deletes.
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

# Tool names the page may call, by route action. Deletions stay with the agent's approval prompt.
READS = {
    "status": "workbench_status",
    "models": "workbench_models",
    "model": "workbench_model",
    "datasets": "workbench_datasets",
    "dataset": "workbench_dataset",
    "runs": "workbench_runs",
    "run": "workbench_run",
    "compare": "workbench_compare",
}
WRITES = {
    "solve": "workbench_solve",
    "evaluate": "workbench_evaluate",
    "cancel": "workbench_cancel",
    "profile_save": "workbench_profile_save",
    "profile_default": "workbench_profile_default",
    "dataset_create": "workbench_dataset_create",
}
STATUS = {"unknown_model": 404, "unknown_dataset": 404, "unknown_run": 404, "unknown_profile": 404,
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
    """The schedule of a run or dataset as a self-contained page for the Desktop's sandboxed frame."""
    package = _package()
    service = importlib.import_module(f"{package.__name__}.service")
    views = importlib.import_module(f"{package.__name__}.views")
    try:
        timeline = package.workbench(_config).schedule(run_id=run or None, dataset_id=dataset or None)
    except service.WorkbenchError as exc:
        return JSONResponse({"ok": False, "code": exc.code, "error": exc.message},
                            status_code=STATUS.get(exc.code, 400))
    return JSONResponse({"ok": True, "document": views.document(timeline)})


@router.get("/{action}")
def read(action: str, model: str = "", dataset: str = "", run: str = "", runs: str = "",
         include_plan: bool = False, max_matches: int = 10) -> JSONResponse:
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
    return _call(READS[action], {k: v for k, v in args.items() if v is not None})


@router.post("/actions/{action}")
def write(action: str, body: Any = Body(default=None)) -> JSONResponse:
    if action not in WRITES:
        return JSONResponse({"ok": False, "code": "unknown_action", "error": f"no action named {action!r}"},
                            status_code=404)
    if not isinstance(body, dict):
        return JSONResponse({"ok": False, "code": "invalid_body", "error": "send a JSON object"}, status_code=400)
    return _call(WRITES[action], body)
