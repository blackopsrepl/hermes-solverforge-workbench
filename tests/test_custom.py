"""Custom models: solverforge-cli projects registered, built and used like stock models.

The fixtures under ``tests/fixtures/custom`` are solverforge-cli projects
(``solverforge new`` plus ``solverforge generate``) with their placeholders
filled in by hand, as an agent leaves them:

- ``crew-tasks``: tasks to crew members (a scalar variable), with constraint
  weights;
- ``route-visits``: visits on van rounds (a list variable), without them.

The integration tests build each project's engine with cargo, so they need a
Rust toolchain and the crates.io registry. Builds share one cargo target dir
under ``engine/target`` and are incremental after the first.
"""

from __future__ import annotations

import json
import shutil
import time
from pathlib import Path

import pytest

from solverforge_workbench_plugin import custom
from solverforge_workbench_plugin import tools as tools_module
from solverforge_workbench_plugin.engine_bin import StockEngine
from solverforge_workbench_plugin.service import Workbench, WorkbenchError
from solverforge_workbench_plugin.store import Store

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests" / "fixtures" / "custom"
TESTKIT = ROOT / "engine" / "target" / "release" / "workbench-testkit"
SHARED_TARGET = ROOT / "engine" / "target" / "custom-models"


def project(tmp_path: Path, name: str) -> Path:
    copy = tmp_path / "projects" / name
    shutil.copytree(FIXTURES / name, copy, ignore=shutil.ignore_patterns("target", "Cargo.lock"))
    return copy


# ---- reading a project ---------------------------------------------------------------


def test_a_scalar_project_is_read_from_its_manifest_and_sources():
    found = custom.inspect(FIXTURES / "crew-tasks")
    assert found.crate == "crew-tasks"
    assert found.solverforge == custom.host_solverforge()
    assert [(r.type_name, r.plural, r.id_field, r.label_field) for r in found.records] == [
        ("Resource", "resources", "id", "name"), ("Task", "tasks", "id", "name")]
    [variable] = found.variables
    assert (variable.entity.type_name, variable.field, variable.kind, variable.target.type_name) == (
        "Task", "resource_idx", "scalar", "Resource")
    assert [(c.name, c.level) for c in found.constraints] == [
        ("assign_every_task", "Hard"), ("skill_match", "Hard"), ("spread_hours", "Soft")]
    assert found.constraints[1].description == "a task goes to a crew member who works its trade."
    assert found.weights_field == "constraint_weights"
    assert found.warnings == []


def test_a_list_project_names_its_element_collection_and_misses_weights():
    found = custom.inspect(FIXTURES / "route-visits")
    [variable] = found.variables
    assert (variable.entity.type_name, variable.field, variable.kind, variable.target.type_name) == (
        "Route", "visits", "list", "Visit")
    assert found.weights_field is None
    assert any("#[constraint_weights]" in w for w in found.warnings)


def test_a_placeholder_description_is_a_warning(tmp_path):
    root = project(tmp_path, "route-visits")
    path = root / "src" / "constraints" / "short_routes.rs"
    path.write_text(path.read_text().replace("/// SOFT: keep rounds even: each route costs the square of its "
                                             "visit count.", "/// SOFT: TODO — describe it."))
    found = custom.inspect(root)
    assert found.constraints[1].level == "Soft"
    assert any("short_routes has no description" in w for w in found.warnings)


def test_a_project_on_another_solverforge_release_is_refused(tmp_path):
    root = project(tmp_path, "crew-tasks")
    cargo = root / "Cargo.toml"
    cargo.write_text(cargo.read_text().replace(f'version = "{custom.host_solverforge()}"', 'version = "0.26.1"'))
    with pytest.raises(custom.CustomError) as error:
        custom.inspect(root)
    assert error.value.code == "version_mismatch"
    assert error.value.detail == {"project": "0.26.1", "workbench": custom.host_solverforge()}


def test_a_directory_without_a_manifest_is_not_a_project(tmp_path):
    with pytest.raises(custom.CustomError) as error:
        custom.inspect(tmp_path)
    assert error.value.code == "not_a_project"


def test_the_tree_hash_follows_sources_and_ignores_build_output(tmp_path):
    root = project(tmp_path, "crew-tasks")
    before = custom.tree_hash(root)
    (root / "target").mkdir()
    (root / "target" / "noise").write_text("x")
    assert custom.tree_hash(root) == before
    (root / "src" / "lib.rs").write_text((root / "src" / "lib.rs").read_text() + "\n")
    assert custom.tree_hash(root) != before


def test_the_generated_engine_points_at_the_project_and_the_bundled_host():
    found = custom.inspect(FIXTURES / "route-visits")
    cargo = custom.render_cargo(found, "route-visits")
    assert f'path = "{FIXTURES / "route-visits"}"' in cargo
    assert f'path = "{custom.HOST_CRATE}"' in cargo
    assert f'version = "={custom.host_solverforge()}"' in cargo
    main = custom.render_main(found, "route-visits", "Route Visits")
    assert "fill::list::<Self>(plan, request, &lists())" in main
    assert "weights_mut" not in main


# ---- registered and used -------------------------------------------------------------


@pytest.fixture
def wb(tmp_path: Path) -> Workbench:
    if not TESTKIT.is_file():
        pytest.fail(f"build the testkit engine first: {TESTKIT} (make test-plugin does)")
    if shutil.which("cargo") is None:
        pytest.fail("custom models are built with cargo; install a Rust toolchain")
    SHARED_TARGET.mkdir(parents=True, exist_ok=True)
    (tmp_path / "custom").mkdir()
    (tmp_path / "custom" / "target").symlink_to(SHARED_TARGET, target_is_directory=True)
    return Workbench(Store(tmp_path / "workbench.db"), tmp_path,
                     lambda: StockEngine(tmp_path, override=str(TESTKIT)), parallel_solves=lambda: 2)


def wait_for(wb: Workbench, run_id: str, limit_s: float = 120) -> dict:
    deadline = time.monotonic() + limit_s
    while time.monotonic() < deadline:
        run = wb.run(run_id)
        if run["status"] not in ("queued", "solving"):
            return run
        time.sleep(0.2)
    raise AssertionError(f"run {run_id} did not finish in {limit_s} s")


def test_a_scalar_project_is_a_model_with_weights_solves_views_and_fill(wb, tmp_path):
    root = project(tmp_path, "crew-tasks")
    registered = wb.register_custom(str(root), name="Crew tasks")
    assert registered["built"] is True and registered["warnings"] == []
    model = registered["model"]
    assert (model["id"], model["kind"], model["name"], model["source"]) == (
        "crew-tasks", "custom", "Crew tasks", str(root))
    assert model["weights"] is True and model["view"] == "assignments" and model["changed"] is False
    assert model["demos"] == ["STANDARD", "SMALL", "LARGE"]
    assert {c["name"]: (c["level"], c["group"]) for c in model["constraints"]} == {
        "skill_match": ("hard", "Hard rules"), "spread_hours": ("soft", "Soft goals"),
        "assign_every_task": ("hard", "Hard rules")}
    # Stock models list first.
    assert [m["id"] for m in wb.models()] == ["toy", "crew-tasks"]

    # Registering an unchanged project again builds nothing.
    assert wb.register_custom(str(root))["built"] is False

    dataset = wb.create_dataset(model_id="crew-tasks", demo="SMALL")
    before = wb.evaluate(dataset["id"])
    assert before["score"] == "-8hard/0soft"
    assert {k["id"]: k["value"] for k in before["kpis"]}["tasks_resource_idx_open"] == 8

    # A profile weights a constraint through the plan's #[constraint_weights] field.
    wb.save_profile(model_id="crew-tasks", name="Assign first", settings={"weights": {"assign_every_task": 3}})
    assert wb.evaluate(dataset["id"], profile="Assign first")["score"] == "-24hard/0soft"

    # Fill ranks crew members for an open task by the model's own score, names them, and applies one.
    options = wb.fill(dataset_id=dataset["id"], items=["task-0"], top=2)
    [item] = options["items"]
    assert (item["id"], item["label"], item["type"]) == ("task-0", "electrical job 0", "Task")
    assert item["options"][0]["label"] in ("Ada", "Dario")
    filled = wb.fill_apply("task-0", item["options"][0]["id"], dataset_id=dataset["id"])
    assert wb.evaluate(filled["id"])["score"] == "-7hard/0soft"

    solve = wb.solve(dataset["id"], overrides={"solve": {"seconds": 3, "unimproved_seconds": 2}})
    run = wait_for(wb, solve["id"])
    assert run["status"] == "completed" and run["feasible"] == 1

    view = wb.view(run_id=run["id"])
    assert view["kind"] == "assignments"
    [board] = view["boards"]
    assert (board["title"], board["item"], board["lane"]) == ("Task by Resource", "Task", "Resource")
    assert [lane["label"] for lane in board["lanes"]] == ["Ada", "Bruno", "Carla"]
    assert sum(len(lane["items"]) for lane in board["lanes"]) == 8 and board["unassigned"] == []

    # A change to the project shows on the model and a new registration rebuilds it.
    lib = root / "src" / "lib.rs"
    lib.write_text(lib.read_text() + "\n")
    assert wb.model("crew-tasks")["changed"] is True
    assert wb.register_custom(str(root))["built"] is True
    assert wb.model("crew-tasks")["changed"] is False


def test_a_list_project_fills_positions_and_draws_its_rounds(wb, tmp_path):
    root = project(tmp_path, "route-visits")
    registered = wb.register_custom(str(root))
    assert registered["model"]["weights"] is False
    assert any("#[constraint_weights]" in w for w in registered["warnings"])
    dataset = wb.create_dataset(model_id="route-visits", demo="SMALL")

    options = wb.fill(dataset_id=dataset["id"], items=["visit-0"], top=3)
    [item] = options["items"]
    assert item["label"] == "Via Roma 1"
    assert [o["id"] for o in item["options"]] == ["route-0@0", "route-1@0", "route-2@0"]
    assert item["options"][0]["label"] == "Van 1, first"
    first = wb.fill_apply("visit-0", "route-1@0", dataset_id=dataset["id"])
    second = wb.fill_apply("visit-1", "route-1@1", dataset_id=first["id"])
    plan = wb.dataset(second["id"], with_plan=True)["plan"]
    assert plan["routes"][1]["visits"] == [0, 1]

    # Moving a placed visit takes it out of its round first.
    moved = wb.fill(dataset_id=second["id"], items=["visit-0"], top=20)
    ids = [o["id"] for o in moved["items"][0]["options"]]
    assert "route-1@0" in ids and "route-1@1" in ids and "route-1@2" not in ids

    with pytest.raises(WorkbenchError) as error:
        wb.save_profile(model_id="route-visits", name="Heavier", settings={"weights": {"short_routes": 2}})
    assert error.value.code == "weights_unsupported"

    solve = wb.solve(dataset["id"], overrides={"solve": {"seconds": 3, "unimproved_seconds": 2}})
    run = wait_for(wb, solve["id"])
    assert run["status"] == "completed"
    # Ten visits on three vans of capacity 4, 5 and 6: the even split 4/3/3 costs 34.
    assert run["score"] == "0hard/-34soft"
    [board] = wb.view(run_id=run["id"])["boards"]
    assert [lane["label"] for lane in board["lanes"]] == ["Van 1", "Van 2", "Van 3"]
    assert sorted(len(lane["items"]) for lane in board["lanes"]) == [3, 3, 4]


def test_registration_refuses_stock_ids_and_another_projects_id(wb, tmp_path):
    root = project(tmp_path, "crew-tasks")
    with pytest.raises(WorkbenchError) as error:
        wb.register_custom(str(root), model_id="toy")
    assert error.value.code == "id_taken"
    wb.register_custom(str(root))
    other = tmp_path / "elsewhere"
    shutil.copytree(root, other)
    with pytest.raises(WorkbenchError) as error:
        wb.register_custom(str(other), model_id="crew-tasks")
    assert error.value.code == "id_taken"


def test_a_build_failure_reports_the_compiler_errors(wb, tmp_path):
    root = project(tmp_path, "route-visits")
    path = root / "src" / "constraints" / "short_routes.rs"
    path.write_text(path.read_text().replace("entity.visits.len() as i64;", "entity.stops.len() as i64;"))
    with pytest.raises(WorkbenchError) as error:
        wb.register_custom(str(root))
    assert error.value.code == "build_failed"
    assert any("no field `stops`" in line for line in error.value.detail["errors"])
    assert wb.store.model("route-visits") is None


def test_removing_a_custom_model_drops_its_data_and_engine_but_not_the_project(wb, tmp_path):
    root = project(tmp_path, "route-visits")
    engine = Path(wb.register_custom(str(root))["model"]["source"])
    wb.create_dataset(model_id="route-visits", demo="SMALL")
    built = tmp_path / "custom" / "route-visits"
    assert built.is_dir()
    with pytest.raises(WorkbenchError) as error:
        wb.remove_custom("toy")
    assert error.value.code == "stock_model"
    wb.remove_custom("route-visits")
    assert wb.store.model("route-visits") is None and wb.datasets("route-visits") == []
    assert not built.exists() and engine.is_dir()


def test_the_agent_tools_register_and_remove(wb, tmp_path):
    root = project(tmp_path, "route-visits")
    table = dict((schema["name"], handler) for schema, handler in tools_module.tool_table(lambda: wb))
    answer = json.loads(table["solverforge_workbench_model_register"]({"path": str(root)}))
    assert answer["ok"] is True and answer["model"]["id"] == "route-visits" and answer["warnings"]
    refused = json.loads(table["solverforge_workbench_model_register"]({"path": str(tmp_path)}))
    assert (refused["ok"], refused["code"]) == (False, "not_a_project")
    assert json.loads(table["solverforge_workbench_model_remove"]({"model": "route-visits"}))["ok"] is True
