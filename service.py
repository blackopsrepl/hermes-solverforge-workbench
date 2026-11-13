"""The workbench: catalog, datasets, profiles and runs over the store and engines.

Everything a tool, slash command or Desktop route does goes through
:class:`Workbench`. It decides nothing about plans: scores, analyses and
KPIs come from the engines; this layer keeps records, applies settings in a
fixed order and queues solves for the detached worker.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable

from . import views
from .engine_bin import ENGINE_VERSION, EngineError, StockEngine, call, engine_env
from .patching import PatchError, apply_patch
from .store import Store, new_id

MAX_DATASET_BYTES = 200 * 1024 * 1024
SECTIONS = ("weights", "params", "solve")
SOLVE_KEYS = ("seconds", "unimproved_seconds", "step_count", "unimproved_step_count", "best_score")
# Settings every model starts from, kept in its read-only Standard profile.
STANDARD_SOLVE = {"seconds": 60, "unimproved_seconds": 20}
MAX_WEIGHT = 1_000_000
TERMINAL = ("completed", "incomplete", "failed", "invalid")


class WorkbenchError(Exception):
    """A refusal with a machine-readable code, a sentence for the user and facts."""

    def __init__(self, code: str, message: str, **detail: Any):
        super().__init__(message)
        self.code = code
        self.message = message
        self.detail = detail


def _merge(*layers: dict[str, Any] | None) -> dict[str, Any]:
    """Merges settings section by section; a later layer wins key by key."""
    merged: dict[str, Any] = {section: {} for section in SECTIONS}
    for layer in layers:
        for section in SECTIONS:
            merged[section].update((layer or {}).get(section) or {})
    return merged


def check_settings(settings: dict[str, Any] | None) -> dict[str, Any]:
    """Validates the shape of a weights/params/solve settings object."""
    settings = settings or {}
    if not isinstance(settings, dict):
        raise WorkbenchError("invalid_settings", "settings must be an object with weights, params and solve")
    unknown = set(settings) - set(SECTIONS)
    if unknown:
        raise WorkbenchError("invalid_settings", f"unknown settings section(s): {', '.join(sorted(unknown))}")
    weights = settings.get("weights") or {}
    if not isinstance(weights, dict) or not all(
            isinstance(name, str) and isinstance(value, int) and not isinstance(value, bool)
            and 0 <= value <= MAX_WEIGHT for name, value in weights.items()):
        raise WorkbenchError("invalid_weights",
                             f"weights map constraint names to whole numbers from 0 to {MAX_WEIGHT}")
    params = settings.get("params") or {}
    if not isinstance(params, dict):
        raise WorkbenchError("invalid_settings", "params must be an object")
    solve = settings.get("solve") or {}
    if not isinstance(solve, dict) or set(solve) - set(SOLVE_KEYS):
        raise WorkbenchError("invalid_settings", f"solve settings are {', '.join(SOLVE_KEYS)}")
    for key, value in solve.items():
        if key == "best_score":
            if value is not None and not isinstance(value, str):
                raise WorkbenchError("invalid_settings", "best_score is a score such as 0hard/0soft")
        elif value is not None and (not isinstance(value, int) or isinstance(value, bool) or value <= 0):
            raise WorkbenchError("invalid_settings", f"{key} must be a positive whole number")
    return {"weights": weights, "params": params, "solve": solve}


class Workbench:
    def __init__(self, store: Store, data_dir: Path, engine: Callable[[], StockEngine],
                 parallel_solves: Callable[[], int] = lambda: 2,
                 on_change: Callable[[str, dict], None] | None = None):
        self.store = store
        self.data_dir = Path(data_dir)
        self._engine = engine
        self._parallel = parallel_solves
        self._on_change = on_change or (lambda what, detail: None)
        self._synced: tuple[str, str] | None = None

    # ---- plumbing -----------------------------------------------------------

    @property
    def engine(self) -> StockEngine:
        return self._engine()

    def changed(self, what: str, **detail: Any) -> None:
        try:
            self._on_change(what, detail)
        except Exception:  # a missing Desktop never fails a tool
            pass

    def engine_for(self, model_id: str) -> Path:
        model = self.model_record(model_id)
        if model["kind"] == "custom":
            path = Path(model["engine"])
            if not path.is_file():
                raise WorkbenchError("engine_missing", f"the engine of {model_id} is missing; register it again")
            return path
        try:
            return self.engine.require()
        except EngineError as exc:
            raise WorkbenchError(exc.code, exc.message) from exc

    # ---- catalog ------------------------------------------------------------

    def sync_stock(self) -> None:
        """Registers the stock models the installed engine carries, once per engine binary."""
        binary = self.engine.binary()
        if binary is None:
            return
        stamp = (str(binary), str(binary.stat().st_mtime_ns))
        if self._synced == stamp:
            return
        found = self.engine.version() or {}
        for listed in found.get("models", []):
            description = call(binary, {"command": "describe", "model": listed["id"]}, timeout_s=60)
            self.store.put_model({"id": listed["id"], "kind": "stock", "name": listed["name"],
                                  "manifest": description, "version": found.get("version", ENGINE_VERSION)})
            self.ensure_standard_profile(listed["id"])
        self._synced = stamp

    def ensure_standard_profile(self, model_id: str) -> None:
        profile_id = f"{model_id}-standard"
        if self.store.profile(profile_id) is None:
            self.store.save_profile({"id": profile_id, "model_id": model_id, "name": "Standard",
                                     "description": "The model's own weights and solve settings.",
                                     "weights": {}, "params": {}, "solve": STANDARD_SOLVE,
                                     "read_only": True, "is_default": False})

    def model_record(self, model_id: str) -> dict[str, Any]:
        self.sync_stock()
        model = self.store.model(model_id)
        if model is None:
            known = [m["id"] for m in self.store.models()]
            raise WorkbenchError("unknown_model", f"no model is named {model_id!r}", models=known)
        return model

    def models(self) -> list[dict[str, Any]]:
        self.sync_stock()
        out = []
        for model in self.store.models():
            description = model["manifest"]
            constraints = description.get("constraints", [])
            out.append({
                "id": model["id"], "kind": model["kind"], "name": model["name"], "version": model["version"],
                "constraints": len(constraints),
                "levels": {level: sum(1 for c in constraints if c.get("level") == level)
                           for level in ("hard", "medium", "soft")},
                "demos": description.get("demos", []), "weights": description.get("weights", False),
                "datasets": (self.store.one("SELECT COUNT(*) AS n FROM datasets WHERE model_id = ?",
                                            model["id"]) or {}).get("n", 0),
            })
        return out

    def model(self, model_id: str) -> dict[str, Any]:
        model = self.model_record(model_id)
        description = model["manifest"]
        return {"id": model["id"], "kind": model["kind"], "name": model["name"], "version": model["version"],
                "source": model.get("source"), "demos": description.get("demos", []),
                "weights": description.get("weights", False),
                "constraints": description.get("constraints", []),
                "profiles": [self.profile_view(p) for p in self.store.profiles(model_id)],
                "default_profile": (self.profile_record(model_id, None) or {}).get("id"),
                "schedule_view": views.has_view(model_id)}

    # ---- datasets -----------------------------------------------------------

    def create_dataset(self, *, model_id: str, name: str = "", tags: list[str] | None = None,
                       demo: str | None = None, plan: Any = None, path: str | None = None,
                       config: dict | None = None) -> dict[str, Any]:
        self.model_record(model_id)
        sources = [demo is not None, plan is not None, path is not None]
        if sum(sources) != 1:
            raise WorkbenchError("one_source", "give exactly one of demo, plan or path")
        origin = "upload"
        if demo is not None:
            answer = self._call(model_id, {"command": "demo", "model": model_id, "demo": demo or None})
            plan, origin, name = answer["plan"], "demo", name or f"Demo {answer['id'].title()}"
        elif path is not None:
            plan = self._read_json(path)
        if not isinstance(plan, dict):
            raise WorkbenchError("invalid_plan", "a dataset is a JSON object")
        dataset = self.store.add_dataset(model_id=model_id, name=name or "Dataset", plan=plan, origin=origin,
                                         tags=_tags(tags), config=check_settings(config))
        self.changed("dataset", id=dataset["id"])
        return self.dataset_view(dataset)

    def _read_json(self, path: str) -> Any:
        file = Path(os.path.expanduser(path))
        if not file.is_file():
            raise WorkbenchError("not_found", f"no file at {file}")
        if file.stat().st_size > MAX_DATASET_BYTES:
            raise WorkbenchError("too_large", f"{file} is larger than {MAX_DATASET_BYTES // (1024 * 1024)} MB")
        try:
            return json.loads(file.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise WorkbenchError("invalid_json", f"{file} is not valid JSON: {exc}") from exc

    def dataset_record(self, dataset_id: str) -> dict[str, Any]:
        dataset = self.store.dataset(dataset_id)
        if dataset is None:
            raise WorkbenchError("unknown_dataset", f"no dataset has id {dataset_id!r}")
        return dataset

    def dataset_view(self, dataset: dict[str, Any], with_plan: bool = False) -> dict[str, Any]:
        view = {key: dataset[key] for key in
                ("id", "model_id", "name", "tags", "origin", "parent_id", "config", "created_at") if key in dataset}
        plan = dataset.get("plan")
        if isinstance(plan, dict):
            view["size"] = {key: len(value) for key, value in plan.items() if isinstance(value, list)}
            if with_plan:
                view["plan"] = plan
        if dataset.get("patch") is not None:
            view["patch"] = dataset["patch"]
        return view

    def datasets(self, model_id: str | None = None, query: str = "", tag: str = "") -> list[dict[str, Any]]:
        out = []
        for dataset in self.store.datasets(model_id, query, tag):
            latest = self.store.runs(dataset_id=dataset["id"], limit=1)
            view = self.dataset_view(dataset)
            view["latest_run"] = self.run_brief(latest[0]) if latest else None
            out.append(view)
        return out

    def dataset(self, dataset_id: str, with_plan: bool = False) -> dict[str, Any]:
        dataset = self.dataset_record(dataset_id)
        view = self.dataset_view(dataset, with_plan)
        view["runs"] = [self.run_brief(run) for run in self.store.runs(dataset_id=dataset_id)]
        view["children"] = [d["id"] for d in self.store.all("SELECT id FROM datasets WHERE parent_id = ?",
                                                            dataset_id)]
        return view

    def patch_dataset(self, dataset_id: str, operations: list[dict], name: str = "",
                      from_run: str | None = None) -> dict[str, Any]:
        """A new dataset: the parent (or a run's best plan) with ``operations`` applied."""
        parent = self.dataset_record(dataset_id)
        base = parent["plan"]
        if from_run:
            run = self.run_record(from_run)
            if run["dataset_id"] != dataset_id or not run.get("plan"):
                raise WorkbenchError("no_plan", f"run {from_run} of this dataset has no plan to start from")
            base = run["plan"]
        try:
            plan = apply_patch(base, operations) if operations else base
        except PatchError as exc:
            raise WorkbenchError("invalid_patch", str(exc)) from exc
        dataset = self.store.add_dataset(
            model_id=parent["model_id"], name=name or f"{parent['name']} (revised)", plan=plan,
            origin="patch" if operations else "run", tags=parent["tags"], parent_id=dataset_id,
            config=parent["config"], patch=operations or None)
        self.changed("dataset", id=dataset["id"])
        return self.dataset_view(dataset)

    def update_dataset(self, dataset_id: str, name: str | None = None, tags: list[str] | None = None) -> dict:
        self.dataset_record(dataset_id)
        dataset = self.store.update_dataset(dataset_id, name=name, tags=None if tags is None else _tags(tags))
        self.changed("dataset", id=dataset_id)
        return self.dataset_view(dataset)  # type: ignore[arg-type]

    def delete_dataset(self, dataset_id: str) -> None:
        self.dataset_record(dataset_id)
        if self.store.runs(dataset_id=dataset_id, status="solving"):
            raise WorkbenchError("busy", "cancel the dataset's running solve before deleting it")
        self.store.delete_dataset(dataset_id)
        self.changed("dataset", id=dataset_id, deleted=True)

    # ---- profiles -----------------------------------------------------------

    def profile_view(self, profile: dict[str, Any]) -> dict[str, Any]:
        return {key: profile[key] for key in ("id", "model_id", "name", "description", "weights", "params",
                                              "solve", "is_default", "read_only", "revision", "updated_at")}

    def profile_record(self, model_id: str, ref: str | None) -> dict[str, Any] | None:
        if not ref:
            self.ensure_standard_profile(model_id)
            return self.store.default_profile(model_id)
        profile = self.store.profile(ref) or self.store.profile_named(model_id, ref)
        if profile is None or profile["model_id"] != model_id:
            names = [p["name"] for p in self.store.profiles(model_id)]
            raise WorkbenchError("unknown_profile", f"model {model_id} has no profile {ref!r}", profiles=names)
        return profile

    def save_profile(self, *, model_id: str, name: str, description: str = "", settings: dict | None = None,
                     profile_id: str | None = None, expected_revision: int | None = None,
                     copy_from: str | None = None) -> dict[str, Any]:
        self.model_record(model_id)
        base = self.profile_record(model_id, copy_from) if copy_from else None
        merged = check_settings(_merge(base, settings) if base else settings)
        self._check_weight_names(model_id, merged["weights"])
        if profile_id is None and self.store.profile_named(model_id, name):
            raise WorkbenchError("name_taken", f"model {model_id} already has a profile named {name!r}")
        if profile_id is None and len(self.store.profiles(model_id)) >= 50:
            raise WorkbenchError("too_many_profiles", "a model keeps at most 50 profiles")
        if profile_id is not None:
            current = self.store.profile(profile_id)
            if current is None or current["model_id"] != model_id:
                raise WorkbenchError("unknown_profile", f"model {model_id} has no profile {profile_id!r}")
            if current["read_only"]:
                raise WorkbenchError("read_only", "the Standard profile is read-only; save a copy instead")
        try:
            profile = self.store.save_profile(
                {"id": profile_id or new_id(), "model_id": model_id, "name": name, "description": description,
                 **merged}, expected_revision=expected_revision)
        except Exception as exc:
            if type(exc).__name__ == "Stale":
                raise WorkbenchError("stale", str(exc)) from exc
            raise
        self.changed("profile", id=profile["id"], model_id=model_id)
        return self.profile_view(profile)

    def _check_weight_names(self, model_id: str, weights: dict[str, int]) -> None:
        known = {c["name"] for c in self.model_record(model_id)["manifest"].get("constraints", [])}
        unknown = sorted(set(weights) - known)
        if unknown:
            raise WorkbenchError("unknown_constraint", f"model {model_id} has no constraint named {unknown[0]!r}",
                                 unknown=unknown, known=sorted(known))

    def set_default_profile(self, model_id: str, ref: str) -> dict[str, Any]:
        profile = self.profile_record(model_id, ref)
        assert profile is not None
        self.store.set_default_profile(model_id, profile["id"])
        self.changed("profile", id=profile["id"], model_id=model_id)
        return self.profile_view(self.store.profile(profile["id"]))  # type: ignore[arg-type]

    def delete_profile(self, model_id: str, ref: str) -> None:
        profile = self.profile_record(model_id, ref)
        assert profile is not None
        if profile["read_only"]:
            raise WorkbenchError("read_only", "the Standard profile cannot be deleted")
        self.store.delete_profile(profile["id"])
        self.changed("profile", id=profile["id"], model_id=model_id, deleted=True)

    def export_profile(self, model_id: str, ref: str) -> dict[str, Any]:
        profile = self.profile_record(model_id, ref)
        assert profile is not None
        return {"format": "solverforge-workbench-profile", "model_id": model_id, "name": profile["name"],
                "description": profile["description"],
                **{section: profile[section] for section in SECTIONS}}

    def import_profile(self, document: dict[str, Any], name: str | None = None) -> dict[str, Any]:
        if not isinstance(document, dict) or document.get("format") != "solverforge-workbench-profile":
            raise WorkbenchError("invalid_profile", "not an exported workbench profile")
        model_id = document.get("model_id", "")
        return self.save_profile(model_id=model_id, name=name or document.get("name") or "Imported",
                                 description=document.get("description", ""),
                                 settings={section: document.get(section) or {} for section in SECTIONS})

    # ---- settings -----------------------------------------------------------

    def effective_settings(self, dataset: dict[str, Any], profile: dict[str, Any] | None,
                           overrides: dict[str, Any] | None) -> dict[str, Any]:
        """Model defaults < parent dataset < profile < overrides < the dataset's own settings."""
        parent = self.store.dataset(dataset["parent_id"]) if dataset.get("parent_id") else None
        return _merge({"solve": STANDARD_SOLVE}, parent and parent["config"], profile, overrides,
                      dataset["config"])

    # ---- runs ---------------------------------------------------------------

    def _call(self, model_id: str, request: dict[str, Any], timeout_s: float = 600) -> dict[str, Any]:
        try:
            return call(self.engine_for(model_id), request, timeout_s=timeout_s)
        except EngineError as exc:
            raise WorkbenchError(exc.code, exc.message, **exc.detail) from exc

    def _prepare_run(self, dataset_id: str, profile_ref: str | None, overrides: dict | None):
        dataset = self.dataset_record(dataset_id)
        model_id = dataset["model_id"]
        profile = self.profile_record(model_id, profile_ref)
        settings = self.effective_settings(dataset, profile, check_settings(overrides))
        self._check_weight_names(model_id, settings["weights"])
        config = {**settings, "engine": str(self.engine_for(model_id))}
        return dataset, profile, config

    def evaluate(self, dataset_id: str, profile: str | None = None, overrides: dict | None = None,
                 plan: dict | None = None) -> dict[str, Any]:
        """Scores the dataset's plan as it stands (or ``plan``, e.g. a run's), without solving it."""
        dataset, profile_row, config = self._prepare_run(dataset_id, profile, overrides)
        body = plan if plan is not None else dataset["plan"]
        if config["params"]:
            body = {**body, "parameters": {**(body.get("parameters") or {}), **config["params"]}}
        run = self.store.add_run(dataset_id=dataset_id, model_id=dataset["model_id"], kind="evaluate",
                                 profile_id=profile_row["id"] if profile_row else None, overrides=overrides or {},
                                 config=config, priority=5, status="solving")
        try:
            answer = self._call(dataset["model_id"], {"command": "evaluate", "model": dataset["model_id"],
                                                      "plan": body, "weights": config["weights"]})
        except WorkbenchError as exc:
            status = "invalid" if exc.code in ("invalid_dataset", "invalid_plan") else "failed"
            self.store.update_run(run["id"], status=status, finished_at=_now(),
                                  error={"code": exc.code, "message": exc.message, **exc.detail})
            self.changed("run", id=run["id"])
            raise
        self.store.update_run(run["id"], status="completed", score=answer["score"],
                              initial_score=answer["score"], feasible=int(bool(answer.get("feasible"))),
                              plan=body, analysis=answer.get("analysis"), kpis=answer.get("kpis"),
                              issues=answer.get("issues"), finished_at=_now())
        self.changed("run", id=run["id"])
        return self.run(run["id"])

    def solve(self, dataset_id: str, profile: str | None = None, overrides: dict | None = None,
              priority: int = 5) -> dict[str, Any]:
        if not isinstance(priority, int) or not 0 <= priority <= 10:
            raise WorkbenchError("invalid_priority", "priority is a whole number from 0 (last) to 10 (first)")
        dataset, profile_row, config = self._prepare_run(dataset_id, profile, overrides)
        run = self.store.add_run(dataset_id=dataset_id, model_id=dataset["model_id"], kind="solve",
                                 profile_id=profile_row["id"] if profile_row else None, overrides=overrides or {},
                                 config=config, priority=priority, status="queued")
        self.kick()
        self.changed("run", id=run["id"])
        return self.run_brief(self.store.run(run["id"], with_plan=False))  # type: ignore[arg-type]

    def reap(self) -> None:
        """Marks solves whose worker died as failed, keeping their best plan."""
        if sys.platform == "win32":
            return
        for run in self.store.runs(status="solving"):
            if run["kind"] != "solve" or not run.get("pid"):
                continue
            try:
                os.kill(run["pid"], 0)
            except ProcessLookupError:
                self.store.update_run(run["id"], status="failed", finished_at=_now(),
                                      error={"code": "solve_interrupted",
                                             "message": "the solve stopped before it finished"})
            except PermissionError:
                pass

    def kick(self) -> None:
        """Starts detached workers until the queue is drained or the parallel limit is reached."""
        self.reap()
        queued = len(self.store.runs(status="queued"))
        active = len({run["pid"] for run in self.store.runs(status="solving") if run["kind"] == "solve"})
        for _ in range(max(0, min(queued, max(1, self._parallel()) - active))):
            subprocess.Popen([sys.executable, str(Path(__file__).parent / "worker.py"), str(self.store.path)],
                             stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                             env=engine_env(), close_fds=True, start_new_session=True)

    def run_record(self, run_id: str) -> dict[str, Any]:
        run = self.store.run(run_id)
        if run is None:
            raise WorkbenchError("unknown_run", f"no run has id {run_id!r}")
        return run

    def run_brief(self, run: dict[str, Any]) -> dict[str, Any]:
        return {key: run.get(key) for key in ("id", "dataset_id", "model_id", "kind", "status", "priority",
                                              "profile_id", "initial_score", "score", "feasible",
                                              "created_at", "started_at", "finished_at", "error")}

    def run(self, run_id: str, with_plan: bool = False, max_matches: int = 3) -> dict[str, Any]:
        self.reap()
        run = self.run_record(run_id)
        view = self.run_brief(run)
        view.update({key: run.get(key) for key in ("config", "overrides", "kpis", "issues", "telemetry")})
        view["config"] = {k: v for k, v in (run.get("config") or {}).items() if k != "engine"}
        view["points"] = self.store.points(run_id)
        view["analysis"] = [_trim(c, max_matches) for c in (run.get("analysis") or [])]
        if with_plan:
            view["plan"] = run.get("plan")
        return view

    def schedule(self, run_id: str | None = None, dataset_id: str | None = None) -> dict[str, Any]:
        """The schedule view of a run's plan (with its broken rules marked) or of a dataset's input."""
        if bool(run_id) == bool(dataset_id):
            raise WorkbenchError("one_source", "give exactly one of run or dataset")
        if run_id:
            run = self.run_record(run_id)
            model_id, plan, analysis = run["model_id"], run.get("plan"), run.get("analysis")
        else:
            dataset = self.dataset_record(dataset_id or "")
            model_id, plan, analysis = dataset["model_id"], dataset["plan"], None
        if not views.has_view(model_id):
            raise WorkbenchError("no_view", f"model {model_id} has no schedule view")
        if not isinstance(plan, dict):
            raise WorkbenchError("no_plan", f"run {run_id} has no plan yet")
        try:
            return views.timeline(model_id, plan, analysis)
        except views.ViewError as exc:
            raise WorkbenchError("invalid_plan", str(exc)) from exc

    def runs(self, dataset_id: str | None = None, model_id: str | None = None,
             status: str | None = None) -> list[dict[str, Any]]:
        self.reap()
        return [self.run_brief(run) for run in self.store.runs(dataset_id=dataset_id, model_id=model_id,
                                                                status=status)]

    def cancel(self, run_id: str) -> dict[str, Any]:
        run = self.run_record(run_id)
        if run["status"] == "queued":
            self.store.update_run(run_id, status="incomplete", cancel_requested=1, finished_at=_now(),
                                  error={"code": "cancelled", "message": "cancelled before it started"})
        elif run["status"] == "solving":
            self.store.update_run(run_id, cancel_requested=1)
        else:
            raise WorkbenchError("finished", f"run {run_id} already finished ({run['status']})")
        self.changed("run", id=run_id)
        return self.run_brief(self.store.run(run_id, with_plan=False))  # type: ignore[arg-type]

    def explain(self, run_id: str, constraint: str = "", entity: str = "", limit: int = 20) -> dict[str, Any]:
        """Matches of one constraint, or every match that involves an entity text."""
        run = self.run_record(run_id)
        analysis = run.get("analysis") or []
        if constraint:
            found = [c for c in analysis if c["name"] == constraint]
            if not found:
                raise WorkbenchError("unknown_constraint", f"run {run_id} has no constraint {constraint!r}",
                                     known=[c["name"] for c in analysis])
            return {"run_id": run_id, "constraints": [_trim(found[0], limit)]}
        if entity:
            needle = entity.lower()
            hits = []
            for item in analysis:
                matches = [m for m in item.get("matches", [])
                           if any(needle in e.get("text", "").lower() for e in m.get("entities", []))]
                if matches:
                    hits.append({**_trim(item, 0), "matches": matches[:limit]})
            return {"run_id": run_id, "entity": entity, "constraints": hits}
        return {"run_id": run_id,
                "constraints": [_trim(c, 0) for c in analysis if c.get("score") and not _zero(c["score"])]}

    def compare(self, run_ids: list[str]) -> dict[str, Any]:
        if len(run_ids) < 2:
            raise WorkbenchError("too_few", "compare needs at least two runs")
        runs = [self.run_record(run_id) for run_id in run_ids]
        names: list[str] = []
        for run in runs:
            for item in run.get("analysis") or []:
                if item["name"] not in names:
                    names.append(item["name"])
        kpi_ids: list[str] = []
        for run in runs:
            for kpi in run.get("kpis") or []:
                if kpi["id"] not in kpi_ids:
                    kpi_ids.append(kpi["id"])
        return {
            "runs": [{**self.run_brief(run),
                      "profile": (self.store.profile(run["profile_id"]) or {}).get("name") if run["profile_id"]
                      else None,
                      "dataset": (self.store.dataset(run["dataset_id"]) or {}).get("name"),
                      "elapsed_ms": (run.get("telemetry") or {}).get("elapsed_ms")} for run in runs],
            "constraints": [{"name": name, "scores": [
                next((c["score"] for c in run.get("analysis") or [] if c["name"] == name), None)
                for run in runs]} for name in names],
            "kpis": [{"id": kpi_id, "values": [
                next((k["value"] for k in run.get("kpis") or [] if k["id"] == kpi_id), None) for run in runs],
                "label": next((k["label"] for run in runs for k in run.get("kpis") or [] if k["id"] == kpi_id), kpi_id),
                "unit": next((k.get("unit", "") for run in runs for k in run.get("kpis") or []
                              if k["id"] == kpi_id), "")} for kpi_id in kpi_ids],
        }

    def status(self) -> dict[str, Any]:
        found = self.engine.version()
        binary = self.engine.binary()
        return {
            "engine": {"installed": binary is not None, "path": str(binary) if binary else None,
                       "version": found and found.get("version"), "expected_version": ENGINE_VERSION,
                       "models": [m["id"] for m in (found or {}).get("models", [])]},
            "queued": len(self.store.runs(status="queued")),
            "solving": len(self.store.runs(status="solving")),
            "parallel_solves": self._parallel(),
        }


def _tags(tags: list[str] | None) -> list[str]:
    if tags is None:
        return []
    if not isinstance(tags, list) or not all(isinstance(tag, str) and 0 < len(tag) <= 64 for tag in tags):
        raise WorkbenchError("invalid_tags", "tags are short strings")
    return sorted(set(tags))


def _trim(item: dict[str, Any], max_matches: int) -> dict[str, Any]:
    return {**item, "matches": (item.get("matches") or [])[:max_matches]}


def _zero(score: str) -> bool:
    return all(part.rstrip("abcdefghijklmnopqrstuvwxyz") in ("0", "0.0", "-0") for part in score.split("/"))


def _now() -> str:
    from .store import now
    return now()
