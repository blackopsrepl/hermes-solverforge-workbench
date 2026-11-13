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
    spec = importlib.util.spec_from_file_location("workbench_plugin_api", ROOT / "dashboard" / "plugin_api.py")
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


def test_the_page_cannot_delete_or_call_unknown_actions(client):
    assert client.post("/actions/dataset_delete", json={"dataset": "x"}).status_code == 404
    assert client.post("/actions/profile_delete", json={"model": "toy", "profile": "x"}).status_code == 404
    assert client.get("/export").status_code == 404
    assert client.post("/actions/solve", json=["not", "an", "object"]).status_code == 400
