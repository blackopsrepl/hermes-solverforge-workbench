"""Finding, verifying and calling engine binaries."""

from __future__ import annotations

import os
import stat
from pathlib import Path

import pytest

from solverforge_workbench_plugin import engine_bin
from solverforge_workbench_plugin.engine_bin import EngineError, StockEngine, call, events

ROOT = Path(__file__).resolve().parents[1]
TESTKIT = ROOT / "engine" / "target" / "release" / "workbench-testkit"


def test_event_lines_are_kept_and_a_model_crates_console_output_is_skipped():
    text = "\n".join([
        " ____        _                 _____",
        '{"event":"started","model":"toy"}',
        "║  Final Score: 0hard/-2soft ║",
        '{"not":"an event"}',
        '{"event":"done","status":"completed"}',
        "{broken",
    ])
    assert [e["event"] for e in events(text)] == ["started", "done"]


def test_an_engine_refusal_becomes_an_error_with_its_code_and_detail():
    with pytest.raises(EngineError) as error:
        call(TESTKIT, {"command": "describe", "model": "nope"})
    assert error.value.code == "unknown_model"
    assert error.value.detail == {"models": ["toy"]}


def test_the_binary_setting_wins_and_a_non_engine_is_not_reported_as_one(tmp_path):
    fake = tmp_path / "not-an-engine"
    fake.write_text("#!/bin/sh\necho hello\n")
    fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
    engine = StockEngine(tmp_path, override=str(fake))
    assert engine.binary() == fake
    assert engine.version() is None
    assert StockEngine(tmp_path, override=str(tmp_path / "missing")).binary() is None
    assert StockEngine(tmp_path, override=str(TESTKIT)).version() == {
        "version": "0.0.0-testkit", "models": [{"id": "toy", "name": "Toy roster"}]}


def test_install_refuses_a_platform_without_a_reviewed_checksum(tmp_path, monkeypatch):
    monkeypatch.setattr(engine_bin, "CHECKSUMS", {})
    with pytest.raises(EngineError) as error:
        StockEngine(tmp_path).install(log=lambda _: None)
    assert error.value.code in ("unpublished_engine", "unsupported_platform")


class _Download:
    def __init__(self, body: bytes):
        self.body = body

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False

    def read(self, limit: int) -> bytes:
        return self.body[:limit]


def test_a_demo_road_network_installs_for_its_demo_only_verified_unpacked_and_once(tmp_path, monkeypatch):
    import gzip
    import hashlib

    network = b'{"version": 3, "nodes": [], "edges": []}'
    body = gzip.compress(network)
    name = "45.0_7.0_45.1_7.1.json"
    monkeypatch.setattr(engine_bin, "DEMO_ROADS", {"routes/CITY": (name, hashlib.sha256(body).hexdigest())})
    asked: list[str] = []

    def urlopen(request, timeout):
        asked.append(request.full_url)
        return _Download(body)

    monkeypatch.setattr(engine_bin.urllib.request, "urlopen", urlopen)
    engine = StockEngine(tmp_path)
    # Another demo, or another model's demo of the same name, downloads nothing.
    assert engine.install_demo_roads("routes", "OTHER", log=lambda _: None) == []
    assert engine.install_demo_roads("other", "CITY", log=lambda _: None) == [] and asked == []
    [path] = engine.install_demo_roads("routes", "CITY", log=lambda _: None)
    assert path == tmp_path / "roads" / name and path.read_bytes() == network
    assert asked == [f"{engine_bin.RELEASE_URL}/roads-{name}.gz"]
    assert engine.install_demo_roads("routes", "CITY", log=lambda _: None) == [] and len(asked) == 1
    path.unlink()
    monkeypatch.setattr(engine_bin, "DEMO_ROADS", {"routes/CITY": (name, "0" * 64)})
    with pytest.raises(EngineError) as error:
        engine.install_demo_roads("routes", "CITY", log=lambda _: None)
    assert error.value.code == "checksum_mismatch" and not path.exists()


def test_engines_start_without_the_callers_secrets(monkeypatch):
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test")
    monkeypatch.setenv("PATH", os.environ.get("PATH", ""))
    env = engine_bin.engine_env()
    assert "OPENAI_API_KEY" not in env
    assert set(env) <= set(engine_bin.ENGINE_ENV_KEYS)
