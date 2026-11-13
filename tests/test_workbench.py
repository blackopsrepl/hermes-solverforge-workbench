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


def test_a_dead_worker_is_reported_instead_of_solving_forever(wb):
    if sys.platform == "win32":
        pytest.skip("process probing is POSIX-only")
    dataset = demo(wb)
    run = wb.store.add_run(dataset_id=dataset["id"], model_id="toy", kind="solve", profile_id=None,
                           overrides={}, config={}, priority=5, status="solving")
    wb.store.update_run(run["id"], pid=2 ** 22 + 12345)
    assert wb.run(run["id"])["status"] == "failed"
    assert wb.run(run["id"])["error"]["code"] == "solve_interrupted"
