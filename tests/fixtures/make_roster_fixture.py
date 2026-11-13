"""Regenerate ess-small-week-run.json: the SMALL_WEEK demo as the stock engine solves and scores it.

Run from the repository root after `make engine`:

    uv run --no-project --python 3.12 python tests/fixtures/make_roster_fixture.py

The demo is solved for a few seconds, then one shift is moved onto an employee
who already works at that time and one is unassigned, and the result is scored.
The fixture keeps the plan and the analysis exactly as a run stores them, so
the schedule view is tested on the engine's real output, including a broken
hard rule and an open shift.
"""

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ENGINE = ROOT / "engine" / "target" / "release" / "solverforge-workbench"
MODEL = "employee-scheduling"


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


demo = next(e for e in call({"command": "demo", "model": MODEL, "demo": "SMALL_WEEK"}) if "plan" in e)
done = next(e for e in call({"command": "solve", "model": MODEL, "plan": demo["plan"],
                             "termination": {"seconds": 5}}) if e["event"] == "done")
plan = done["plan"]
shifts = plan["shifts"]
free = [s for s in shifts if not s.get("pinned") and s.get("employee")]
# Move a shift onto someone already working a shift that overlaps it.
moved = next(a for a in free for b in shifts
             if b is not a and b.get("employee") and b["employee"] != a["employee"]
             and b["start"] < a["end"] and a["start"] < b["end"])
other = next(b for b in shifts if b is not moved and b.get("employee") and b["employee"] != moved["employee"]
             and b["start"] < moved["end"] and moved["start"] < b["end"])
moved["employee"] = other["employee"]
open_shift = next(s for s in free if s is not moved)
open_shift["employee"] = None
scored = next(e for e in call({"command": "evaluate", "model": MODEL, "plan": plan, "max_matches": 100})
              if "analysis" in e)
fixture = {"model_id": MODEL, "plan": plan, "analysis": scored["analysis"], "score": scored["score"],
           "moved": moved["id"], "opened": open_shift["id"]}
(Path(__file__).parent / "ess-small-week-run.json").write_text(json.dumps(fixture, indent=1) + "\n")
print(scored["score"], moved["id"], "->", other["employee"], "| open:", open_shift["id"])
