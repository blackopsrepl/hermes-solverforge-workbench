"""The Desktop backend: reads and the button actions, over the testkit engine."""

from __future__ import annotations

import importlib
import importlib.util
from pathlib import Path

import pytest

fastapi = pytest.importorskip("fastapi")
from fastapi import FastAPI  # noqa: E402
from fastapi.testclient import TestClient  # noqa: E402


ROOT = Path(__file__).resolve().parents[1]
TESTKIT = ROOT / "engine" / "target" / "release" / "workbench-testkit"


@pytest.fixture
def client(tmp_path, monkeypatch):
    spec = importlib.util.spec_from_file_location("solverforge_workbench_plugin_api", ROOT / "dashboard" / "plugin_api.py")
    assert spec is not None and spec.loader is not None
    api = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(api)
    # The backend loads its own copy of the package, as in Hermes; build the workbench from that copy
    # so its refusals are the classes the backend's handlers catch.
    package = api._package()
    modules = {name: importlib.import_module(f"{package.__name__}.{name}") for name in ("engine_bin", "service", "store")}
    bench = modules["service"].Workbench(modules["store"].Store(tmp_path / "w.db"), tmp_path,
                                         lambda: modules["engine_bin"].StockEngine(tmp_path, override=str(TESTKIT)))
    monkeypatch.setattr(package, "workbench", lambda config=None: bench)
    monkeypatch.setattr(api, "_config", lambda key, default=None: default)
    app = FastAPI()
    app.include_router(api.router)
    return TestClient(app)


def test_reads_and_button_actions_reach_the_workbench(client):
    assert client.get("/models").json()["models"][0]["id"] == "toy"
    created = client.post("/actions/dataset_create", json={"model": "toy", "demo": ""}).json()
    dataset = created["dataset"]["id"]
    run = client.post("/actions/evaluate", json={"dataset": dataset}).json()["run"]
    assert run["score"] == "-6hard/0soft"
    got = client.get("/run", params={"run": run["id"], "max_matches": 2}).json()["run"]
    assert len(got["analysis"][0]["matches"]) == 2
    assert client.get("/dataset", params={"dataset": dataset}).json()["dataset"]["runs"][0]["id"] == run["id"]


def test_refusals_keep_their_code_and_a_fitting_status(client):
    missing = client.get("/run", params={"run": "nope"})
    assert missing.status_code == 404 and missing.json()["code"] == "unknown_run"
    assert client.get("/model", params={"model": "nope"}).status_code == 404


def test_the_page_deletes_only_after_its_confirm_dialog_asked(client):
    dataset = client.post("/actions/dataset_create", json={"model": "toy", "demo": "", "name": "Week"}).json()["dataset"]
    client.post("/actions/evaluate", json={"dataset": dataset["id"]})
    asked = client.get("/confirm", params={"action": "dataset_delete", "dataset": dataset["id"]}).json()
    assert asked["text"] == "Delete the dataset “Week” and its 1 run?"
    refused = client.post("/actions/dataset_delete", json={"dataset": dataset["id"]})
    assert refused.status_code == 400 and refused.json()["code"] == "confirm_required"
    assert client.get("/dataset", params={"dataset": dataset["id"]}).status_code == 200
    assert client.post("/actions/dataset_delete", json={"dataset": dataset["id"], "confirm": "yes"}).status_code == 400
    assert client.post("/actions/dataset_delete", json={"dataset": dataset["id"], "confirm": True}).json()["ok"]
    assert client.get("/dataset", params={"dataset": dataset["id"]}).status_code == 404
    assert client.get("/confirm", params={"action": "solve"}).status_code == 404


def test_the_page_renames_exports_and_imports(client, tmp_path):
    dataset = client.post("/actions/dataset_create", json={"model": "toy", "demo": ""}).json()["dataset"]
    renamed = client.post("/actions/dataset_update", json={"dataset": dataset["id"], "name": "Spring", "tags": ["a"]}).json()
    assert renamed["dataset"]["name"] == "Spring" and renamed["dataset"]["tags"] == ["a"]
    target = tmp_path / "spring.json"
    assert client.post("/actions/export", json={"dataset": dataset["id"], "path": str(target)}).json()["path"] == str(target)
    assert "shifts" in target.read_text()
    saved = client.post("/actions/profile_save", json={"model": "toy", "name": "Lenient",
                                                       "settings": {"weights": {"Unassigned shift": 0}}}).json()["profile"]
    exported = tmp_path / "lenient.json"
    client.post("/actions/profile_export", json={"model": "toy", "profile": saved["id"], "path": str(exported)})
    import json
    document = json.loads(exported.read_text())
    copy = client.post("/actions/profile_import", json={"document": document, "name": "Lenient copy"}).json()["profile"]
    assert copy["weights"] == {"Unassigned shift": 0}
    both = client.post("/actions/profile_import", json={"document": document, "path": str(exported)})
    assert both.json()["code"] == "one_source"


def test_unknown_actions_and_bodies_are_refused(client):
    assert client.post("/actions/nope", json={}).status_code == 404
    assert client.get("/export").status_code == 404
    assert client.post("/actions/solve", json=["not", "an", "object"]).status_code == 400
