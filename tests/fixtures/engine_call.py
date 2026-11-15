"""One request to the locally built stock engine, for the fixture generators."""

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ENGINE = ROOT / "engine" / "target" / "release" / "solverforge-workbench"


def call(request: dict) -> list[dict]:
    # The engine reads a closed stdin as "cancel", so it stays open until the last event.
    engine = subprocess.Popen([str(ENGINE)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    assert engine.stdin is not None and engine.stdout is not None
    engine.stdin.write(json.dumps(request) + "\n")
    engine.stdin.flush()
    events = []
    for line in engine.stdout:
        if line.startswith("{") and '"event"' in line:
            events.append(json.loads(line))
            # A solve ends with "done"; every other command answers with one event.
            if events[-1]["event"] == "done" or request["command"] != "solve":
                break
    engine.stdin.close()
    engine.wait(timeout=30)
    return events


def models() -> list[dict]:
    """The engine's models, as `--version` lists them."""
    out = subprocess.run([str(ENGINE), "--version"], capture_output=True, text=True, timeout=20)
    return next(json.loads(line) for line in out.stdout.splitlines() if '"version"' in line)["models"]
