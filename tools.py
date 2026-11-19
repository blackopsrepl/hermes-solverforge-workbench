"""Tool handlers: each takes the agent's arguments and returns a JSON string."""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any, Callable

from .engine_bin import EngineError
from .schemas import SCHEMAS
from .service import Workbench, WorkbenchError, guard_write


def ok(**payload: Any) -> str:
    return json.dumps({"ok": True, **payload}, ensure_ascii=False)


def fail(code: str, message: str, **detail: Any) -> str:
    return json.dumps({"ok": False, "code": code, "error": message, **detail}, ensure_ascii=False)


def _write_json(path: str, value: Any) -> str:
    target = Path(os.path.expanduser(path))
    if target.suffix.lower() != ".json":
        raise WorkbenchError("invalid_path", "export paths end in .json")
    guard_write(target)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")
    return str(target)


class Tools:
    def __init__(self, workbench: Callable[[], Workbench]):
        self._workbench = workbench

    @property
    def wb(self) -> Workbench:
        return self._workbench()

    def status(self, args: dict) -> str:
        return ok(**self.wb.status())

    def models(self, args: dict) -> str:
        return ok(models=self.wb.models())

    def model(self, args: dict) -> str:
        return ok(model=self.wb.model(args["model"]))

    def model_register(self, args: dict) -> str:
        started = self.wb.start_register(args["path"], args.get("model"), args.get("name"))
        if started["build"]:
            started["next"] = "follow the build with solverforge_workbench_model_build until it is done or failed"
        return ok(**started)

    def model_build(self, args: dict) -> str:
        if args.get("build"):
            return ok(build=self.wb.build(args["build"]))
        return ok(builds=self.wb.builds(args.get("model")))

    def model_remove(self, args: dict) -> str:
        self.wb.remove_custom(args["model"])
        return ok(removed=args["model"])

    def dataset_create(self, args: dict) -> str:
        dataset = self.wb.create_dataset(model_id=args["model"], name=args.get("name", ""), tags=args.get("tags"),
                                         demo=args.get("demo"), plan=args.get("plan"), path=args.get("path"),
                                         config=args.get("settings"))
        return ok(dataset=dataset)

    def dataset_check(self, args: dict) -> str:
        if ("plan" in args) == ("path" in args):
            raise WorkbenchError("one_source", "give exactly one of plan or path")
        return ok(**self.wb.check_dataset(args["model"], plan=args.get("plan"), path=args.get("path")))

    def datasets(self, args: dict) -> str:
        return ok(datasets=self.wb.datasets(args.get("model"), args.get("query", ""), args.get("tag", "")))

    def dataset(self, args: dict) -> str:
        return ok(dataset=self.wb.dataset(args["dataset"], bool(args.get("include_plan"))))

    def dataset_update(self, args: dict) -> str:
        return ok(dataset=self.wb.update_dataset(args["dataset"], args.get("name"), args.get("tags")))

    def dataset_patch(self, args: dict) -> str:
        if not (args.get("operations") or args.get("edits") or args.get("from_run")):
            raise WorkbenchError("no_change", "give edits, operations or from_run")
        return ok(dataset=self.wb.patch_dataset(args["dataset"], args.get("operations") or [],
                                                args.get("name", ""), args.get("from_run"), args.get("edits"),
                                                in_place=bool(args.get("in_place"))))

    def changes(self, args: dict) -> str:
        return ok(changes=self.wb.changes(args["run"]))

    def edits(self, args: dict) -> str:
        return ok(edits=self.wb.edits(args.get("dataset"), args.get("run")))

    def dataset_delete(self, args: dict) -> str:
        self.wb.delete_dataset(args["dataset"])
        return ok(deleted=args["dataset"])

    def profile_save(self, args: dict) -> str:
        profile = self.wb.save_profile(model_id=args["model"], name=args["name"],
                                       description=args.get("description", ""), settings=args.get("settings"),
                                       profile_id=args.get("profile"),
                                       expected_revision=args.get("expected_revision"),
                                       copy_from=args.get("copy_from"))
        return ok(profile=profile)

    def profile_default(self, args: dict) -> str:
        return ok(profile=self.wb.set_default_profile(args["model"], args["profile"]))

    def profile_delete(self, args: dict) -> str:
        self.wb.delete_profile(args["model"], args["profile"])
        return ok(deleted=args["profile"])

    def profile_export(self, args: dict) -> str:
        path = _write_json(args["path"], self.wb.export_profile(args["model"], args["profile"]))
        return ok(path=path)

    def profile_import(self, args: dict) -> str:
        if ("path" in args) == ("document" in args):
            raise WorkbenchError("one_source", "give exactly one of path or document")
        document = self.wb._read_json(args["path"]) if "path" in args else args["document"]
        return ok(profile=self.wb.import_profile(document, args.get("name")))

    def solve(self, args: dict) -> str:
        run = self.wb.solve(args["dataset"], args.get("profile"), args.get("overrides"), args.get("priority", 5))
        return ok(run=run, next="call solverforge_workbench_run with this run id to follow the solve")

    def evaluate(self, args: dict) -> str:
        return ok(run=self.wb.evaluate(args["dataset"], args.get("profile"), args.get("overrides")))

    def run(self, args: dict) -> str:
        return ok(run=self.wb.run(args["run"], bool(args.get("include_plan")), int(args.get("max_matches", 3))))

    def runs(self, args: dict) -> str:
        return ok(runs=self.wb.runs(args.get("dataset"), args.get("model"), args.get("status")))

    def run_update(self, args: dict) -> str:
        return ok(run=self.wb.rename_run(args["run"], args.get("label", "")))

    def cancel(self, args: dict) -> str:
        return ok(run=self.wb.cancel(args["run"]))

    def explain(self, args: dict) -> str:
        return ok(**self.wb.explain(args["run"], args.get("constraint", ""), args.get("entity", ""),
                                    int(args.get("limit", 20))))

    def fill(self, args: dict) -> str:
        return ok(**self.wb.fill(run_id=args.get("run"), dataset_id=args.get("dataset"), items=args.get("items"),
                                 top=args.get("top", 5), profile=args.get("profile")))

    def fill_apply(self, args: dict) -> str:
        dataset = self.wb.fill_apply(args["item"], args["value"], run_id=args.get("run"),
                                     dataset_id=args.get("dataset"), name=args.get("name", ""),
                                     profile=args.get("profile"))
        return ok(dataset=dataset, next="solve or evaluate the new dataset, or fill its next open item")

    def experiment_create(self, args: dict) -> str:
        experiment = self.wb.create_experiment(datasets=args["datasets"], profiles=args["profiles"],
                                               repeats=args.get("repeats", 1), name=args.get("name", ""),
                                               overrides=args.get("overrides"), priority=args.get("priority", 3),
                                               judge=args.get("judge"))
        return ok(experiment=experiment, next="call solverforge_workbench_experiment to follow it and read the ranking")

    def experiment(self, args: dict) -> str:
        return ok(experiment=self.wb.experiment(args["experiment"]))

    def experiments(self, args: dict) -> str:
        return ok(experiments=self.wb.experiments(args.get("model")))

    def experiment_cancel(self, args: dict) -> str:
        return ok(experiment=self.wb.cancel_experiment(args["experiment"]))

    def experiment_delete(self, args: dict) -> str:
        self.wb.delete_experiment(args["experiment"])
        return ok(deleted=args["experiment"])

    def trends(self, args: dict) -> str:
        return ok(**self.wb.trends(args["model"]))

    def compare(self, args: dict) -> str:
        return ok(**self.wb.compare(args["runs"]))

    def export(self, args: dict) -> str:
        if bool(args.get("run")) == bool(args.get("dataset")):
            raise WorkbenchError("one_source", "give exactly one of run or dataset")
        if args.get("run"):
            run = self.wb.run_record(args["run"])
            if not run.get("plan"):
                raise WorkbenchError("no_plan", f"run {args['run']} has no plan yet")
            value = run["plan"]
        else:
            value = self.wb.dataset_record(args["dataset"])["plan"]
        return ok(path=_write_json(args["path"], value))


def handler(method: Callable[[dict], str]) -> Callable[..., str]:
    def call(args: dict | None = None, **_: Any) -> str:
        try:
            return method(args or {})
        except WorkbenchError as exc:
            return fail(exc.code, exc.message, **exc.detail)
        except EngineError as exc:
            return fail(exc.code, exc.message, **exc.detail)
        except KeyError as exc:
            return fail("missing_argument", f"missing argument {exc.args[0]!r}")
        except (ValueError, OSError) as exc:
            return fail("invalid_argument", str(exc))
    return call


def tool_table(workbench: Callable[[], Workbench]) -> list[tuple[dict, Callable[..., str]]]:
    tools = Tools(workbench)
    table = []
    for schema in SCHEMAS:
        method = getattr(tools, schema["name"].removeprefix("solverforge_workbench_"))
        table.append((schema, handler(method)))
    return table
