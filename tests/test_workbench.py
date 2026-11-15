"""The workbench end to end on the testkit engine: a real SolverForge model in a real process.

Run ``make test-plugin`` (it builds ``engine/target/release/workbench-testkit``
first). The toy roster has three nurses, six shifts on three days and three
constraints: every shift needs a nurse (hard), a nurse works once a day (hard)
and the first nurse is avoided (soft, one per shift).
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

import pytest

from solverforge_workbench_plugin import tools as tools_module
from solverforge_workbench_plugin.engine_bin import StockEngine
from solverforge_workbench_plugin.service import Workbench, WorkbenchError
from solverforge_workbench_plugin.store import Store

ROOT = Path(__file__).resolve().parents[1]
TESTKIT = ROOT / "engine" / "target" / "release" / "workbench-testkit"


@pytest.fixture
def wb(tmp_path: Path) -> Workbench:
    if not TESTKIT.is_file():
        pytest.fail(f"build the testkit engine first: {TESTKIT} (make test-plugin does)")
    events: list[tuple[str, dict]] = []
    bench = Workbench(Store(tmp_path / "workbench.db"), tmp_path,
                      lambda: StockEngine(tmp_path, override=str(TESTKIT)),
                      parallel_solves=lambda: 2, on_change=lambda what, detail: events.append((what, detail)))
    bench.events = events  # type: ignore[attr-defined]
    return bench


def wait_for(wb: Workbench, run_id: str, limit_s: float = 60) -> dict:
    deadline = time.monotonic() + limit_s
    while time.monotonic() < deadline:
        run = wb.run(run_id)
        if run["status"] not in ("queued", "solving"):
            return run
        time.sleep(0.2)
    raise AssertionError(f"run {run_id} did not finish in {limit_s} s: {wb.run(run_id)['status']}")


def demo(wb: Workbench, **kwargs) -> dict:
    return wb.create_dataset(model_id="toy", demo="", **kwargs)


def test_the_engines_models_join_the_catalog_with_a_read_only_standard_profile(wb):
    models = wb.models()
    assert [m["id"] for m in models] == ["toy"]
    assert models[0]["levels"] == {"hard": 2, "medium": 0, "soft": 1}
    model = wb.model("toy")
    assert model["constraints"][0]["group"] == "Coverage"
    assert [(p["name"], p["read_only"]) for p in model["profiles"]] == [("Standard", 1)]
    with pytest.raises(WorkbenchError) as error:
        wb.save_profile(model_id="toy", name="Standard", profile_id="toy-standard",
                        settings={"weights": {"Unassigned shift": 2}})
    assert error.value.code == "read_only"


def test_a_demo_dataset_is_stored_with_its_size(wb):
    dataset = demo(wb, tags=["week", "demo"])
    assert dataset["origin"] == "demo"
    assert dataset["size"] == {"nurses": 3, "shifts": 6}
    assert wb.datasets(model_id="toy", tag="week")[0]["id"] == dataset["id"]
    assert wb.datasets(query="nothing-like-this") == []


def test_evaluate_scores_the_plan_as_it_stands(wb):
    run = wb.evaluate(demo(wb)["id"])
    assert run["status"] == "completed"
    assert run["score"] == "-6hard/0soft"
    assert run["feasible"] == 0
    assert run["analysis"][0]["match_count"] == 6
    assert len(run["analysis"][0]["matches"]) == 3


def test_a_profile_weight_reaches_the_engine_and_a_misspelt_one_is_refused(wb):
    dataset = demo(wb)
    wb.save_profile(model_id="toy", name="Lenient", settings={"weights": {"Unassigned shift": 0}})
    assert wb.evaluate(dataset["id"], profile="Lenient")["score"] == "0hard/0soft"
    with pytest.raises(WorkbenchError) as error:
        wb.save_profile(model_id="toy", name="Typo", settings={"weights": {"Unasigned shift": 2}})
    assert error.value.code == "unknown_constraint"


def test_settings_apply_in_order_and_the_datasets_own_settings_win(wb):
    dataset = demo(wb, config={"weights": {"Unassigned shift": 3}})
    wb.save_profile(model_id="toy", name="Doubled", settings={"weights": {"Unassigned shift": 2}})
    run = wb.evaluate(dataset["id"], profile="Doubled", overrides={"weights": {"Unassigned shift": 5}})
    assert run["config"]["weights"] == {"Unassigned shift": 3}
    assert run["score"] == "-18hard/0soft"
    plain = demo(wb)
    run = wb.evaluate(plain["id"], profile="Doubled", overrides={"weights": {"Unassigned shift": 5}})
    assert run["score"] == "-30hard/0soft"


def test_a_solve_runs_in_the_background_and_keeps_its_score_history(wb):
    dataset = demo(wb)
    queued = wb.solve(dataset["id"], overrides={"solve": {"seconds": 2}})
    assert queued["status"] in ("queued", "solving")
    run = wait_for(wb, queued["id"])
    assert run["status"] == "completed", run
    assert run["initial_score"] == "-6hard/0soft"
    assert run["score"] == "0hard/0soft"
    assert run["feasible"] == 1
    assert run["kpis"][0] == {"id": "assigned", "label": "Assigned shifts", "value": 6.0, "unit": "shifts"}
    assert run["points"] and run["points"][-1]["score"] == "0hard/0soft"
    plan = wb.run(run["id"], with_plan=True)["plan"]
    assert all(shift["nurse_idx"] in (1, 2) for shift in plan["shifts"])


def test_cancelling_a_solve_keeps_the_best_plan(wb):
    dataset = demo(wb)
    queued = wb.solve(dataset["id"], overrides={"solve": {"seconds": 120, "unimproved_seconds": None}})
    deadline = time.monotonic() + 30
    while wb.run(queued["id"])["status"] != "solving" and time.monotonic() < deadline:
        time.sleep(0.1)
    time.sleep(1.5)
    wb.cancel(queued["id"])
    run = wait_for(wb, queued["id"], limit_s=30)
    assert run["status"] == "incomplete"
    assert run["feasible"] == 1
    assert wb.run(run["id"], with_plan=True)["plan"]["shifts"]


def test_a_dataset_error_marks_the_solve_invalid_with_the_engines_issues(wb):
    dataset = wb.create_dataset(model_id="toy", plan={
        "nurses": [{"id": 0, "name": "solo"}],
        "shifts": [{"id": 0, "day": 0, "nurse_idx": 4}],
    })
    run = wait_for(wb, wb.solve(dataset["id"])["id"])
    assert run["status"] == "invalid"
    assert run["error"]["code"] == "invalid_dataset"
    assert run["error"]["issues"][0]["code"] == "unknown_nurse"


def test_a_patch_makes_a_child_dataset_and_leaves_the_parent_alone(wb):
    parent = demo(wb)
    child = wb.patch_dataset(parent["id"], [
        {"op": "replace", "path": "/shifts/[id=0]/nurse_idx", "value": 1},
        {"op": "replace", "path": "/shifts/[id=0]/pinned", "value": True},
        {"op": "remove", "path": "/shifts/[id=5]"},
    ], name="Pinned Monday")
    assert child["parent_id"] == parent["id"]
    assert child["size"]["shifts"] == 5
    assert wb.dataset(parent["id"])["children"] == [child["id"]]
    assert wb.dataset_record(parent["id"])["plan"]["shifts"][0]["nurse_idx"] is None
    with pytest.raises(WorkbenchError) as error:
        wb.patch_dataset(parent["id"], [{"op": "replace", "path": "/shifts/[id=99]/day", "value": 1}])
    assert error.value.code == "invalid_patch"


def test_replanning_starts_from_a_solved_run(wb):
    dataset = demo(wb)
    run = wait_for(wb, wb.solve(dataset["id"], overrides={"solve": {"seconds": 1}})["id"])
    child = wb.patch_dataset(dataset["id"], [{"op": "replace", "path": "/shifts/[id=0]/pinned", "value": True}],
                             from_run=run["id"])
    plan = wb.dataset_record(child["id"])["plan"]
    assert plan["shifts"][0]["pinned"] is True
    assert all(shift["nurse_idx"] is not None for shift in plan["shifts"])


def test_explain_and_compare_read_the_engines_analysis(wb):
    dataset = demo(wb)
    before = wb.evaluate(dataset["id"])
    after = wait_for(wb, wb.solve(dataset["id"], overrides={"solve": {"seconds": 1}})["id"])
    explained = wb.explain(before["id"], constraint="Unassigned shift", limit=10)
    assert len(explained["constraints"][0]["matches"]) == 6
    by_entity = wb.explain(before["id"], entity="id: 3,")
    assert by_entity["constraints"][0]["matches"][0]["entities"][0]["type"] == "Shift"
    compared = wb.compare([before["id"], after["id"]])
    unassigned = next(row for row in compared["constraints"] if row["name"] == "Unassigned shift")
    assert unassigned["scores"] == ["-6hard/0soft", "0hard/0soft"]
    assert compared["kpis"][0]["values"] == [0.0, 6.0]


def test_profiles_export_and_import(wb, tmp_path):
    wb.save_profile(model_id="toy", name="Strict", settings={"weights": {"Shift for the first nurse": 4},
                                                            "solve": {"seconds": 5}})
    exported = wb.export_profile("toy", "Strict")
    path = tmp_path / "strict.json"
    path.write_text(json.dumps(exported))
    imported = wb.import_profile(json.loads(path.read_text()), name="Strict copy")
    assert imported["weights"] == {"Shift for the first nurse": 4}
    assert imported["solve"] == {"seconds": 5}


def test_tools_return_codes_and_messages_the_agent_can_act_on(wb):
    table = dict((schema["name"], handler) for schema, handler in tools_module.tool_table(lambda: wb))
    answer = json.loads(table["solverforge_workbench_model"]({"model": "nope"}))
    assert answer == {"ok": False, "code": "unknown_model", "error": "no model is named 'nope'", "models": ["toy"]}
    created = json.loads(table["solverforge_workbench_dataset_create"]({"model": "toy", "demo": ""}))
    assert created["ok"] and created["dataset"]["size"]["shifts"] == 6
    missing = json.loads(table["solverforge_workbench_solve"]({}))
    assert missing["code"] == "missing_argument"


def test_every_schema_has_a_handler_and_the_manifest_lists_it():
    names = [schema["name"] for schema, _ in tools_module.tool_table(lambda: None)]  # type: ignore[arg-type]
    manifest = (ROOT / "plugin.yaml").read_text()
    for name in names:
        assert f"  - {name}\n" in manifest
    assert len(names) == len(set(names))


def test_a_dead_worker_is_reported_instead_of_solving_forever(wb):
    if sys.platform == "win32":
        pytest.skip("process probing is POSIX-only")
    dataset = demo(wb)
    run = wb.store.add_run(dataset_id=dataset["id"], model_id="toy", kind="solve", profile_id=None,
                           overrides={}, config={}, priority=5, status="solving")
    wb.store.update_run(run["id"], pid=2 ** 22 + 12345)
    assert wb.run(run["id"])["status"] == "failed"
    assert wb.run(run["id"])["error"]["code"] == "solve_interrupted"


def test_fill_ranks_each_open_shift_and_applying_one_makes_a_child_dataset(wb):
    dataset = demo(wb)
    options = wb.fill(dataset_id=dataset["id"], top=2)
    assert options["source"] == f"dataset {dataset['id']}"
    assert [item["id"] for item in options["items"]] == [f"Shift#{n}" for n in range(6)]
    best = options["items"][0]["options"][0]
    # Any nurse but the avoided first one: the shift is covered and no soft point is lost.
    assert best["id"] in ("Nurse#1", "Nurse#2")
    assert best["delta"] == "1hard/0soft"
    assert best["changes"] == [{"constraint": "Unassigned shift", "level": "hard", "delta": "1hard/0soft"}]
    child = wb.fill_apply("Shift#0", best["id"], dataset_id=dataset["id"])
    assert child["parent_id"] == dataset["id"] and child["origin"] == "fill"
    assert child["patch"] == [{"op": "fill", "item": "Shift#0", "value": best["id"]}]
    plan = wb.dataset_record(child["id"])["plan"]
    assert plan["shifts"][0]["nurse_idx"] == int(best["id"].split("#")[1])
    assert [item["id"] for item in wb.fill(dataset_id=child["id"])["items"]] == [f"Shift#{n}" for n in range(1, 6)]
    with pytest.raises(WorkbenchError) as error:
        wb.fill_apply("Shift#1", "Nurse#7", dataset_id=dataset["id"])
    assert error.value.code == "unknown_value"


def test_fill_reads_a_runs_plan(wb):
    dataset = demo(wb)
    run = wait_for(wb, wb.solve(dataset["id"], overrides={"solve": {"seconds": 1}})["id"])
    # A solved plan has no open shift; naming one ranks its alternatives.
    assert wb.fill(run_id=run["id"])["items"] == []
    moved = wb.fill(run_id=run["id"], items=["Shift#2"], top=3)["items"][0]["options"]
    assert len(moved) == 3 and moved[0]["feasible"] is True


def test_parameters_must_be_ones_the_model_lists(wb):
    assert wb.model("toy")["parameters"] == []
    with pytest.raises(WorkbenchError) as error:
        wb.save_profile(model_id="toy", name="Fast", settings={"params": {"speed": 40}})
    assert error.value.code == "unknown_parameter"


def test_freezing_needs_a_date_time_and_a_model_with_time(wb):
    dataset = demo(wb)
    with pytest.raises(WorkbenchError) as error:
        wb.evaluate(dataset["id"], overrides={"solve": {"freeze_before": "Monday"}})
    assert error.value.code == "invalid_settings"
    with pytest.raises(WorkbenchError) as error:
        wb.evaluate(dataset["id"], overrides={"solve": {"freeze_before": "2027-03-01T06:00"}})
    assert error.value.code == "freeze_unsupported"


def test_a_model_without_a_plan_view_says_so(wb):
    dataset = demo(wb)
    assert wb.model("toy")["view"] is None
    with pytest.raises(WorkbenchError) as error:
        wb.view(dataset_id=dataset["id"])
    assert error.value.code == "no_view"


def test_an_experiment_solves_every_dataset_under_every_profile_and_ranks_the_profiles(wb):
    first, second = demo(wb), demo(wb, name="Second")
    # Weight 0 switches the soft rule off: that profile cannot tell the first nurse from the others,
    # and only the judge (Standard) sees the difference.
    wb.save_profile(model_id="toy", name="Blind", settings={"weights": {"Shift for the first nurse": 0}})
    experiment = wb.create_experiment(datasets=[first["id"], second["id"]], profiles=["Standard", "Blind"],
                                      repeats=2, overrides={"solve": {"seconds": 1}})
    assert experiment["judge_name"] == "Standard"
    assert sum(experiment["statuses"].values()) == 8
    deadline = time.monotonic() + 120
    while not wb.experiment(experiment["id"])["done"] and time.monotonic() < deadline:
        time.sleep(0.3)
    done = wb.experiment(experiment["id"])
    assert done["done"] and done["statuses"] == {"completed": 8}
    assert len(done["cells"]) == 4 and all(c["finished"] == 2 and c["feasible"] == 2 for c in done["cells"])
    standard = [c for c in done["cells"] if c["profile_id"] == "toy-standard"]
    assert all(c["best"]["score"] == c["worst"] == "0hard/0soft" for c in standard)
    assert [(r["profile"], r["place"]) for r in done["ranking"]] == [("Standard", 1), ("Blind", 2)]
    assert done["ranking"][0]["average_place"] == 1.0
    assert wb.experiments("toy")[0]["runs"] == 8
    trends = wb.trends("toy")
    assert len(trends["solves"]) == 8 and trends["feasible_share"] == 1.0
    assert trends["kpis"][0]["id"] == "assigned" and len(trends["kpis"][0]["points"]) == 8
    wb.delete_experiment(experiment["id"])
    assert wb.experiments("toy") == [] and wb.runs(model_id="toy") == []


def test_an_experiment_refuses_mixed_models_and_too_many_solves(wb):
    dataset = demo(wb)
    with pytest.raises(WorkbenchError) as error:
        wb.create_experiment(datasets=[dataset["id"]], profiles=["Standard"], repeats=21)
    assert error.value.code == "invalid_repeats"
    with pytest.raises(WorkbenchError) as error:
        wb.create_experiment(datasets=[dataset["id"]], profiles=["Nope"])
    assert error.value.code == "unknown_profile"


def test_a_v0_1_database_migrates_keeping_its_records(tmp_path):
    import sqlite3

    path = tmp_path / "old.db"
    db = sqlite3.connect(path)
    db.executescript((ROOT / "tests" / "fixtures" / "store-v0.1.0.sql").read_text())
    db.execute("INSERT INTO datasets (id, model_id, name, origin, plan, created_at)"
               " VALUES ('d1', 'toy', 'Old', 'demo', '{}', '2026-11-01')")
    db.execute("INSERT INTO runs (id, dataset_id, model_id, kind, status, created_at)"
               " VALUES ('r1', 'd1', 'toy', 'solve', 'completed', '2026-11-01')")
    db.commit()
    db.close()
    store = Store(path)
    assert store.dataset("d1")["name"] == "Old"
    assert store.run("r1")["experiment_id"] is None
    child = store.add_dataset(model_id="toy", name="Filled", plan={}, origin="fill", parent_id="d1")
    assert child["origin"] == "fill"
    store.delete_dataset("d1")  # the rebuilt table keeps its cascade to runs
    assert store.run("r1") is None
