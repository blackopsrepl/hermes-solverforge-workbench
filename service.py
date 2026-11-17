"""The workbench: catalog, datasets, profiles and runs over the store and engines.

Everything a tool, slash command or Desktop route does goes through
:class:`Workbench`. It decides nothing about plans: scores, analyses and
KPIs come from the engines; this layer keeps records, applies settings in a
fixed order and queues solves for the detached worker.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable

from . import custom
from .engine_bin import ENGINE_VERSION, EngineError, StockEngine, call, engine_env
from .patching import PatchError, apply_patch
from .store import Store, new_id

MAX_DATASET_BYTES = 200 * 1024 * 1024
SECTIONS = ("weights", "params", "solve")
SOLVE_KEYS = ("seconds", "unimproved_seconds", "step_count", "unimproved_step_count", "best_score",
              "freeze_before")
LOCAL_DATE_TIME = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$")
MAX_EXPERIMENT_RUNS = 200
SCORE_PART = re.compile(r"(-?\d+(?:\.\d+)?)([a-z]*)", re.IGNORECASE)
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
        elif key == "freeze_before":
            if value is not None and not (isinstance(value, str) and LOCAL_DATE_TIME.match(value)):
                raise WorkbenchError("invalid_settings",
                                     "freeze_before is a local date-time such as 2027-03-03T06:00")
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
                "source": model.get("source"), "changed": self._changed_since_registration(model),
                "view": description.get("view"), "constraints": len(constraints),
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
                "source": model.get("source"), "changed": self._changed_since_registration(model),
                "demos": description.get("demos", []),
                "weights": description.get("weights", False),
                "parameters": description.get("parameters", []),
                "kpis": description.get("kpis", []),
                "view": description.get("view"),
                "constraints": description.get("constraints", []),
                "profiles": [self.profile_view(p) for p in self.store.profiles(model_id)],
                "default_profile": (self.profile_record(model_id, None) or {}).get("id")}

    # ---- custom models --------------------------------------------------------

    def _custom_version(self, project_root: Path) -> str:
        """The project's tree hash and the hash of the code that hosts it: a
        change to either needs a new engine."""
        host = custom.tree_hash(custom.HOST_CRATE)
        generator = hashlib.sha256(Path(custom.__file__).read_bytes()).hexdigest()
        return hashlib.sha256(f"{custom.tree_hash(project_root)}:{host}:{generator}".encode()).hexdigest()

    def register_custom(self, path: str, model_id: str | None = None, name: str | None = None) -> dict[str, Any]:
        """Builds a solverforge-cli project's engine and registers it as a model;
        builds again only when the project or the workbench changed."""
        try:
            project = custom.inspect(Path(os.path.expanduser(path)))
        except custom.CustomError as exc:
            raise WorkbenchError(exc.code, exc.message, **exc.detail) from exc
        model_id = model_id or custom.default_id(project)
        if not custom.ID_PATTERN.match(model_id):
            raise WorkbenchError("invalid_id", f"{model_id!r} is not a model id: use 2-48 lowercase letters, "
                                 "digits and dashes, starting with a letter")
        self.sync_stock()
        existing = self.store.model(model_id)
        if existing and existing["kind"] == "stock":
            raise WorkbenchError("id_taken", f"{model_id} is a stock model; register the project under another id")
        if existing and existing.get("source") != str(project.root):
            raise WorkbenchError("id_taken", f"{model_id} is the project at {existing['source']}; register this "
                                 "one under another id, or remove that model first", source=existing["source"])
        name = name or (existing["name"] if existing else project.crate.replace("-", " ").replace("_", " ").title())
        version = self._custom_version(project.root)
        current = (existing is not None and existing["version"] == version and existing["name"] == name
                   and Path(existing["engine"] or "").is_file())
        if not current:
            try:
                engine = custom.build(project, model_id, name, self.data_dir / "custom")
            except custom.CustomError as exc:
                raise WorkbenchError(exc.code, exc.message, **exc.detail) from exc
            description = call(engine, {"command": "describe", "model": model_id}, timeout_s=60)
            self.store.put_model({"id": model_id, "kind": "custom", "name": name, "manifest": description,
                                  "engine": str(engine), "source": str(project.root), "version": version})
            self.ensure_standard_profile(model_id)
            self.changed("models", model=model_id)
        return {"model": self.model(model_id), "built": not current, "warnings": project.warnings}

    def remove_custom(self, model_id: str) -> None:
        """Forgets a custom model with its datasets, runs, profiles and engine;
        the project itself is left as it is."""
        model = self.model_record(model_id)
        if model["kind"] != "custom":
            raise WorkbenchError("stock_model", f"{model_id} is a stock model; only custom models can be removed")
        busy = [r["id"] for r in self.store.runs(model_id=model_id) if r["status"] in ("queued", "solving")]
        if busy:
            raise WorkbenchError("model_busy", f"{model_id} has {len(busy)} solve(s) queued or running; cancel them "
                                 "first", runs=busy)
        self.store.delete_model(model_id)
        shutil.rmtree(self.data_dir / "custom" / model_id, ignore_errors=True)
        self.changed("models", model=model_id)

    def _changed_since_registration(self, model: dict[str, Any]) -> bool | None:
        source = model.get("source")
        if model["kind"] != "custom" or not source or not Path(source).is_dir():
            return None
        return self._custom_version(Path(source)) != model["version"]

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
        self._check_params(model_id, merged["params"])
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
        manifest = self.model_record(model_id)["manifest"]
        if weights and not manifest.get("weights", False):
            raise WorkbenchError("weights_unsupported", f"model {model_id} has no #[constraint_weights] field, so its "
                                 "constraint weights are fixed; a profile can set only its solve settings and parameters")
        known = {c["name"] for c in manifest.get("constraints", [])}
        unknown = sorted(set(weights) - known)
        if unknown:
            raise WorkbenchError("unknown_constraint", f"model {model_id} has no constraint named {unknown[0]!r}",
                                 unknown=unknown, known=sorted(known))

    def _check_params(self, model_id: str, params: dict[str, Any]) -> None:
        """Every parameter must be one the model lists, with a value of its kind."""
        listed = {p["name"]: p for p in self.model_record(model_id)["manifest"].get("parameters", [])}
        unknown = sorted(set(params) - set(listed))
        if unknown:
            raise WorkbenchError("unknown_parameter", f"model {model_id} has no parameter named {unknown[0]!r}",
                                 unknown=unknown, known=sorted(listed))
        for name, value in params.items():
            kind = listed[name].get("kind")
            fits = {
                "integer": isinstance(value, int) and not isinstance(value, bool),
                "duration": isinstance(value, int) and not isinstance(value, bool),
                "number": isinstance(value, (int, float)) and not isinstance(value, bool),
                "boolean": isinstance(value, bool),
                "string": isinstance(value, str),
            }.get(kind, True)
            if not fits:
                raise WorkbenchError("invalid_parameter", f"parameter {name!r} of {model_id} is a {kind}",
                                     parameter=name, kind=kind)

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
        self._check_params(model_id, settings["params"])
        config = {**settings, "engine": str(self.engine_for(model_id))}
        return dataset, profile, config

    def _request(self, command: str, model_id: str, plan: dict[str, Any], config: dict[str, Any],
                 **extra: Any) -> dict[str, Any]:
        """One engine request for ``plan`` under the effective settings ``config``."""
        return {"command": command, "model": model_id, "plan": with_params(plan, config.get("params")),
                "weights": config.get("weights", {}),
                **({"freeze_before": config["solve"]["freeze_before"]}
                   if (config.get("solve") or {}).get("freeze_before") else {}),
                **extra}

    def evaluate(self, dataset_id: str, profile: str | None = None, overrides: dict | None = None,
                 plan: dict | None = None) -> dict[str, Any]:
        """Scores the dataset's plan as it stands (or ``plan``, e.g. a run's), without solving it."""
        dataset, profile_row, config = self._prepare_run(dataset_id, profile, overrides)
        body = plan if plan is not None else dataset["plan"]
        run = self.store.add_run(dataset_id=dataset_id, model_id=dataset["model_id"], kind="evaluate",
                                 profile_id=profile_row["id"] if profile_row else None, overrides=overrides or {},
                                 config=config, priority=5, status="solving")
        try:
            answer = self._call(dataset["model_id"], self._request("evaluate", dataset["model_id"], body, config))
        except WorkbenchError as exc:
            status = "invalid" if exc.code in ("invalid_dataset", "invalid_plan") else "failed"
            self.store.update_run(run["id"], status=status, finished_at=_now(),
                                  error={"code": exc.code, "message": exc.message, **exc.detail})
            self.changed("run", id=run["id"])
            raise
        self.store.update_run(run["id"], status="completed", score=answer["score"],
                              initial_score=answer["score"], feasible=int(bool(answer.get("feasible"))),
                              plan=with_params(body, config["params"]), analysis=answer.get("analysis"),
                              kpis=answer.get("kpis"),
                              issues=answer.get("issues"), finished_at=_now())
        self.changed("run", id=run["id"])
        return self.run(run["id"])

    def solve(self, dataset_id: str, profile: str | None = None, overrides: dict | None = None,
              priority: int = 5, experiment_id: str | None = None, kick: bool = True) -> dict[str, Any]:
        if not isinstance(priority, int) or not 0 <= priority <= 10:
            raise WorkbenchError("invalid_priority", "priority is a whole number from 0 (last) to 10 (first)")
        dataset, profile_row, config = self._prepare_run(dataset_id, profile, overrides)
        run = self.store.add_run(dataset_id=dataset_id, model_id=dataset["model_id"], kind="solve",
                                 profile_id=profile_row["id"] if profile_row else None, overrides=overrides or {},
                                 config=config, priority=priority, status="queued", experiment_id=experiment_id)
        if kick:
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
                                              "profile_id", "experiment_id", "initial_score", "score", "feasible",
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

    def _source(self, run_id: str | None, dataset_id: str | None) -> tuple[dict[str, Any], dict[str, Any], str]:
        """(dataset, plan, what) of a run's plan or a dataset's input; exactly one is given."""
        if bool(run_id) == bool(dataset_id):
            raise WorkbenchError("one_source", "give exactly one of run or dataset")
        if run_id:
            run = self.run_record(run_id)
            if not isinstance(run.get("plan"), dict):
                raise WorkbenchError("no_plan", f"run {run_id} has no plan yet")
            return self.dataset_record(run["dataset_id"]), run["plan"], f"run {run_id}"
        dataset = self.dataset_record(dataset_id or "")
        return dataset, dataset["plan"], f"dataset {dataset_id}"

    def view(self, run_id: str | None = None, dataset_id: str | None = None) -> dict[str, Any]:
        """The plan drawn by its model (a timeline, or routes on a map), broken rules marked.

        A run's plan is drawn under the settings it was solved with; a dataset's
        input under its model's defaults and the dataset's own settings.
        """
        dataset, plan, _ = self._source(run_id, dataset_id)
        model_id = dataset["model_id"]
        if not self.model_record(model_id)["manifest"].get("view"):
            raise WorkbenchError("no_view", f"model {model_id} has no plan view")
        config = self.run_record(run_id)["config"] if run_id else self.effective_settings(dataset, None, None)
        # The stored plan of a run already carries its parameters; a frozen solve's pins are in the plan.
        config = {**config, "solve": {k: v for k, v in (config.get("solve") or {}).items() if k != "freeze_before"}}
        return self._call(model_id, self._request("view", model_id, plan, config))["view"]

    def fill(self, run_id: str | None = None, dataset_id: str | None = None, items: list[str] | None = None,
             top: int = 5, profile: str | None = None) -> dict[str, Any]:
        """The best places for open items of a plan, ranked by the score each reaches, with the
        constraints each placement changes."""
        if not isinstance(top, int) or isinstance(top, bool) or not 1 <= top <= 50:
            raise WorkbenchError("invalid_top", "top is a whole number from 1 to 50")
        if items is not None and not (isinstance(items, list) and all(isinstance(i, str) for i in items)):
            raise WorkbenchError("invalid_items", "items is a list of item ids")
        dataset, plan, what = self._source(run_id, dataset_id)
        config = self._fill_config(dataset, profile)
        answer = self._call(dataset["model_id"], self._request(
            "fill", dataset["model_id"], plan, config, fill={"items": items or [], "top": top}))
        return {"source": what, "dataset_id": dataset["id"], "run_id": run_id, "items": answer["items"],
                "issues": answer.get("issues", [])}

    def fill_apply(self, item: str, value: str, run_id: str | None = None, dataset_id: str | None = None,
                   name: str = "", profile: str | None = None) -> dict[str, Any]:
        """A new dataset: the plan with one fill option applied, its parent the dataset it came from."""
        if not (isinstance(item, str) and item and isinstance(value, str) and value):
            raise WorkbenchError("invalid_choice", "give the item id and the value id of a fill option")
        dataset, plan, _ = self._source(run_id, dataset_id)
        config = self._fill_config(dataset, profile)
        answer = self._call(dataset["model_id"], self._request(
            "fill", dataset["model_id"], plan, config, fill={"apply": {"item": item, "value": value}}))
        child = self.store.add_dataset(
            model_id=dataset["model_id"], name=name or f"{dataset['name']} (filled)", plan=answer["plan"],
            origin="fill", tags=dataset["tags"], parent_id=dataset["id"], config=dataset["config"],
            patch=[{"op": "fill", "item": item, "value": value, **({"from_run": run_id} if run_id else {})}])
        self.changed("dataset", id=child["id"])
        return self.dataset_view(child)

    def _fill_config(self, dataset: dict[str, Any], profile: str | None) -> dict[str, Any]:
        settings = self.effective_settings(dataset, self.profile_record(dataset["model_id"], profile), None)
        self._check_weight_names(dataset["model_id"], settings["weights"])
        self._check_params(dataset["model_id"], settings["params"])
        return settings

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
        """Matches of one constraint, or every match that names an entity by its id or in its label."""
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

            def names(e: dict[str, Any]) -> bool:
                return str(e.get("id", "")).lower() == needle or needle in str(e.get("label", "")).lower()

            hits = []
            for item in analysis:
                matches = [m for m in item.get("matches", []) if any(names(e) for e in m.get("entities", []))]
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

    # ---- experiments ----------------------------------------------------------

    def create_experiment(self, *, datasets: list[str], profiles: list[str], repeats: int = 1, name: str = "",
                          overrides: dict | None = None, priority: int = 3,
                          judge: str | None = None) -> dict[str, Any]:
        """Solves every dataset under every profile, ``repeats`` times each, to rank the profiles.

        Profiles weigh constraints differently, so their own scores do not compare: every plan
        is scored again under one judge profile (the model's Standard profile unless named).
        """
        if not isinstance(datasets, list) or not datasets or not all(isinstance(d, str) for d in datasets):
            raise WorkbenchError("invalid_datasets", "datasets is a non-empty list of dataset ids")
        if not isinstance(profiles, list) or not profiles or not all(isinstance(p, str) for p in profiles):
            raise WorkbenchError("invalid_profiles", "profiles is a non-empty list of profile ids or names")
        if not isinstance(repeats, int) or isinstance(repeats, bool) or not 1 <= repeats <= 20:
            raise WorkbenchError("invalid_repeats", "repeats is a whole number from 1 to 20")
        rows = [self.dataset_record(d) for d in dict.fromkeys(datasets)]
        models = {row["model_id"] for row in rows}
        if len(models) != 1:
            raise WorkbenchError("mixed_models", "an experiment's datasets all belong to one model",
                                 models=sorted(models))
        model_id = models.pop()
        chosen = []
        for ref in dict.fromkeys(profiles):
            profile = self.profile_record(model_id, ref)
            assert profile is not None
            if profile["id"] not in chosen:
                chosen.append(profile["id"])
        self.ensure_standard_profile(model_id)
        judge_row = self.profile_record(model_id, judge or f"{model_id}-standard")
        assert judge_row is not None
        total = len(rows) * len(chosen) * repeats
        if total > MAX_EXPERIMENT_RUNS:
            raise WorkbenchError("too_many_runs", f"the experiment would queue {total} solves; at most "
                                                  f"{MAX_EXPERIMENT_RUNS}", runs=total)
        overrides = check_settings(overrides)
        for dataset in rows:  # refuse before queuing anything
            for profile_id in chosen:
                self._prepare_run(dataset["id"], profile_id, overrides)
        experiment = self.store.add_experiment(
            model_id=model_id, name=name or f"Experiment {len(self.store.experiments(model_id)) + 1}",
            datasets=[row["id"] for row in rows], profiles=chosen, judge=judge_row["id"], repeats=repeats,
            overrides=overrides)
        for _ in range(repeats):
            for dataset in rows:
                for profile_id in chosen:
                    self.solve(dataset["id"], profile_id, overrides, priority, experiment_id=experiment["id"],
                               kick=False)
        self.kick()
        self.changed("experiment", id=experiment["id"])
        return self.experiment(experiment["id"])

    def experiment_record(self, experiment_id: str) -> dict[str, Any]:
        experiment = self.store.experiment(experiment_id)
        if experiment is None:
            raise WorkbenchError("unknown_experiment", f"no experiment has id {experiment_id!r}")
        return experiment

    def experiment(self, experiment_id: str) -> dict[str, Any]:
        """Every dataset × profile cell with its runs, and the profiles ranked: most feasible
        solves first, then the best average place across datasets, then the best median score."""
        self.reap()
        experiment = self.experiment_record(experiment_id)
        self._judge(experiment)
        runs = self.store.runs(experiment_id=experiment_id, limit=None)
        names = {d: (self.store.dataset(d) or {}).get("name", d) for d in experiment["datasets"]}
        profile_names = {p: (self.store.profile(p) or {}).get("name", p) for p in experiment["profiles"]}
        cells = []
        for dataset_id in experiment["datasets"]:
            for profile_id in experiment["profiles"]:
                mine = [r for r in runs if r["dataset_id"] == dataset_id and r["profile_id"] == profile_id]
                cells.append(_cell(dataset_id, profile_id, mine))
        # Each dataset ranks the profiles by their best solve; a profile's place is averaged over datasets.
        places: dict[str, list[int]] = {p: [] for p in experiment["profiles"]}
        for dataset_id in experiment["datasets"]:
            ranked = sorted((c for c in cells if c["dataset_id"] == dataset_id and c["best"]),
                            key=lambda c: _rank_key(c["best"]))
            for place, cell in enumerate(ranked, 1):
                places[cell["profile_id"]].append(place)
        ranking = []
        for profile_id in experiment["profiles"]:
            mine = [c for c in cells if c["profile_id"] == profile_id]
            finished = sum(c["finished"] for c in mine)
            feasible = sum(c["feasible"] for c in mine)
            medians = [c["median"] for c in mine if c["median"]]
            ranking.append({
                "profile_id": profile_id, "profile": profile_names[profile_id], "finished": finished,
                "feasible": feasible, "runs": sum(c["runs"] for c in mine),
                "average_place": round(statistics.mean(places[profile_id]), 2) if places[profile_id] else None,
                "median_scores": medians,
            })
        ranking.sort(key=lambda r: (-(r["feasible"] / r["finished"]) if r["finished"] else 1,
                                    r["average_place"] if r["average_place"] is not None else float("inf"),
                                    [_negated(_score_key(m)) for m in r["median_scores"]]))
        for place, row in enumerate(ranking, 1):
            row["place"] = place
            row.pop("median_scores")
        statuses: dict[str, int] = {}
        for run in runs:
            statuses[run["status"]] = statuses.get(run["status"], 0) + 1
        return {**experiment, "judge_name": (self.store.profile(experiment["judge"]) or {}).get("name"),
                "dataset_names": names, "profile_names": profile_names, "statuses": statuses,
                "done": all(r["status"] in TERMINAL and not _awaiting_judge(r) for r in runs),
                "cells": cells, "ranking": ranking}

    def _judge(self, experiment: dict[str, Any]) -> None:
        """Scores every finished plan of the experiment not yet judged under the judge profile."""
        judge = self.store.profile(experiment["judge"])
        for run in self.store.runs(experiment_id=experiment["id"], limit=None):
            if run["status"] not in ("completed", "incomplete") or run.get("judged_score") or not run.get("score"):
                continue
            plan = (self.store.run(run["id"]) or {}).get("plan")
            dataset = self.store.dataset(run["dataset_id"])
            if not isinstance(plan, dict) or dataset is None:
                continue
            settings = self.effective_settings(dataset, judge, None)
            answer = self._call(run["model_id"], self._request("evaluate", run["model_id"], plan,
                                                               {**settings, "solve": {}}, max_matches=0))
            self.store.update_run(run["id"], judged_score=answer["score"],
                                  judged_feasible=int(bool(answer.get("feasible"))))

    def experiments(self, model_id: str | None = None) -> list[dict[str, Any]]:
        out = []
        for experiment in self.store.experiments(model_id):
            runs = self.store.runs(experiment_id=experiment["id"], limit=None)
            out.append({**experiment, "runs": len(runs),
                        "finished": sum(1 for r in runs if r["status"] in TERMINAL),
                        "done": all(r["status"] in TERMINAL for r in runs)})
        return out

    def cancel_experiment(self, experiment_id: str) -> dict[str, Any]:
        self.experiment_record(experiment_id)
        for run in self.store.runs(experiment_id=experiment_id, limit=None):
            if run["status"] in ("queued", "solving"):
                self.cancel(run["id"])
        return self.experiment(experiment_id)

    def delete_experiment(self, experiment_id: str) -> None:
        self.experiment_record(experiment_id)
        if self.store.runs(experiment_id=experiment_id, status="solving"):
            raise WorkbenchError("busy", "cancel the experiment's running solves before deleting it")
        self.store.delete_experiment(experiment_id)
        self.changed("experiment", id=experiment_id, deleted=True)

    # ---- trends -------------------------------------------------------------

    def trends(self, model_id: str) -> dict[str, Any]:
        """Every finished solve of a model in time order, with its score and KPIs, and each KPI's series."""
        manifest = self.model_record(model_id)["manifest"]
        solves = self.store.finished_solves(model_id)
        names = {}
        points = []
        for run in solves:
            if run["dataset_id"] not in names:
                names[run["dataset_id"]] = (self.store.dataset(run["dataset_id"]) or {}).get("name", run["dataset_id"])
            points.append({"run_id": run["id"], "dataset_id": run["dataset_id"], "dataset": names[run["dataset_id"]],
                           "profile_id": run["profile_id"], "experiment_id": run["experiment_id"],
                           "finished_at": run["finished_at"], "score": run["score"],
                           "feasible": bool(run["feasible"]),
                           "kpis": {k["id"]: k["value"] for k in run.get("kpis") or []
                                    if isinstance(k.get("value"), (int, float))}})
        catalog = {k["id"]: k for k in manifest.get("kpis", [])}
        seen = []
        for run in solves:
            for kpi in run.get("kpis") or []:
                if kpi["id"] not in seen:
                    seen.append(kpi["id"])
                    catalog.setdefault(kpi["id"], {"id": kpi["id"], "label": kpi.get("label", kpi["id"]),
                                                   "unit": kpi.get("unit", "")})
        series = [{**catalog[kpi_id], "points": [[p["finished_at"], p["kpis"][kpi_id], p["run_id"]]
                                                  for p in points if kpi_id in p["kpis"]]} for kpi_id in seen]
        return {"model_id": model_id, "solves": points, "kpis": series,
                "feasible_share": round(sum(p["feasible"] for p in points) / len(points), 3) if points else None}

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


def with_params(plan: dict[str, Any], params: dict[str, Any] | None) -> dict[str, Any]:
    """The plan with the settings' model parameters merged into its own ``parameters``."""
    if not params:
        return plan
    return {**plan, "parameters": {**(plan.get("parameters") or {}), **params}}


def _score_key(score: str | None) -> tuple[float, ...]:
    """A score as numbers, most significant level first: "-1hard/0medium/-30soft" -> (-1, 0, -30)."""
    return tuple(float(value) for value, _ in SCORE_PART.findall(score or ""))


def _negated(key: tuple[float, ...]) -> tuple[float, ...]:
    return tuple(-value for value in key)


def _rank_key(run: dict[str, Any]) -> tuple:
    """Sorts feasible solves before infeasible ones, then higher scores first."""
    return (0 if run.get("feasible") else 1, _negated(_score_key(run.get("score"))))


def _cell(dataset_id: str, profile_id: str, runs: list[dict[str, Any]]) -> dict[str, Any]:
    """One dataset x profile cell; its scores are the judge's, so cells compare."""
    finished = [{**r, "own_score": r["score"], "score": r["judged_score"], "feasible": r["judged_feasible"]}
                for r in runs if r["status"] in ("completed", "incomplete") and r.get("judged_score")]
    ordered = sorted(finished, key=_rank_key)
    elapsed = [(r.get("telemetry") or {}).get("elapsed_ms") for r in finished]
    kpis: dict[str, list[float]] = {}
    for run in finished:
        for kpi in run.get("kpis") or []:
            if isinstance(kpi.get("value"), (int, float)):
                kpis.setdefault(kpi["id"], []).append(kpi["value"])
    return {
        "dataset_id": dataset_id, "profile_id": profile_id, "runs": len(runs), "finished": len(finished),
        "feasible": sum(1 for r in finished if r.get("feasible")),
        "best": {"run_id": ordered[0]["id"], "score": ordered[0]["score"],
                 "feasible": bool(ordered[0].get("feasible"))} if ordered else None,
        "median": ordered[(len(ordered) - 1) // 2]["score"] if ordered else None,
        "worst": ordered[-1]["score"] if ordered else None,
        "mean_elapsed_ms": round(statistics.mean([e for e in elapsed if e is not None]))
        if any(e is not None for e in elapsed) else None,
        "kpis": {kpi_id: round(statistics.mean(values), 3) for kpi_id, values in kpis.items()},
        "run_ids": [r["id"] for r in runs],
    }


def _awaiting_judge(run: dict[str, Any]) -> bool:
    """A finished plan the judge has not scored yet: it ended after this read's judging pass."""
    return run["status"] in ("completed", "incomplete") and bool(run.get("score")) and not run.get("judged_score")


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
