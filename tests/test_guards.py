"""The limits on what the agent's tools may build, read and write."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

import solverforge_workbench_plugin as plugin
from solverforge_workbench_plugin.engine_bin import StockEngine
from solverforge_workbench_plugin.service import Workbench
from solverforge_workbench_plugin.store import Store
from solverforge_workbench_plugin.tools import tool_table

TESTKIT = Path(__file__).resolve().parents[1] / "engine" / "target" / "release" / "workbench-testkit"


class Ctx:
    def __init__(self):
        self.hooks = {}

    def register_tool(self, **_kw):
        pass

    def register_hook(self, name, fn):
        self.hooks[name] = fn

    def register_command(self, *_a, **_kw):
        pass

    def register_cli_command(self, **_kw):
        pass

    def register_skill(self, *_a, **_kw):
        pass

    def get_config(self, _key, default=None):
        return default


@pytest.fixture
def gate(tmp_path, monkeypatch):
    monkeypatch.setenv("HERMES_HOME", str(tmp_path / "home"))
    ctx = Ctx()
    plugin.register(ctx)
    return ctx.hooks["pre_tool_call"]


@pytest.fixture
def table(tmp_path):
    if not TESTKIT.is_file():
        pytest.fail(f"build the testkit engine first: {TESTKIT} (make test-plugin does)")
    bench = Workbench(Store(tmp_path / "workbench.db"), tmp_path, lambda: StockEngine(tmp_path, override=str(TESTKIT)))
    return {schema["name"]: handler for schema, handler in tool_table(lambda: bench)}


@pytest.fixture
def guard():
    module = sys.modules["agent.file_safety"]
    if not hasattr(module, "DENIED_READS"):
        pytest.skip("Hermes's own file guard is installed")
    yield module
    module.DENIED_READS.clear()
    module.DENIED_WRITES.clear()


def test_registering_a_project_asks_for_that_project(gate, tmp_path):
    one, two = tmp_path / "one", tmp_path / "two"
    asked = gate(tool_name="solverforge_workbench_model_register", args={"path": str(one / ".." / "one")})
    assert asked["action"] == "approve"
    assert asked["rule_key"] == f"solverforge_workbench_model_register:{one.resolve()}"
    assert str(one.resolve()) in asked["message"] and "runs the project's own code" in asked["message"]
    other = gate(tool_name="solverforge_workbench_model_register", args={"path": str(two)})
    assert other["rule_key"] != asked["rule_key"]
    assert gate(tool_name="solverforge_workbench_solve", args={"dataset": "x"}) is None

