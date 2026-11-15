"""Regenerate ess-month-run.json and ess-catalog.json: the LARGE_MONTH demo as the stock engine solves and scores it.

Run from the repository root after `make engine`:

    uv run --no-project --python 3.12 python tests/fixtures/make_roster_fixture.py

The demo is solved for 30 seconds, then one shift is moved onto an employee
who already works at that time and one is unassigned, and the result is scored.
The fixture keeps the plan and the analysis exactly as a run stores them, so
the schedule view is tested on the engine's real output, including a broken
hard rule and an open shift.
"""

import json
from pathlib import Path

from engine_call import call

MODEL = "employee-scheduling"

demo = next(e for e in call({"command": "demo", "model": MODEL, "demo": "LARGE_MONTH"}) if "plan" in e)
done = next(e for e in call({"command": "solve", "model": MODEL, "plan": demo["plan"],
                             "termination": {"seconds": 30}}) if e["event"] == "done")
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
scored = next(e for e in call({"command": "evaluate", "model": MODEL, "plan": plan, "max_matches": 20})
              if "analysis" in e)
fixture = {"model_id": MODEL, "plan": plan, "analysis": scored["analysis"], "score": scored["score"],
           "moved": moved["id"], "opened": open_shift["id"]}
(Path(__file__).parent / "ess-month-run.json").write_text(json.dumps(fixture, indent=1) + "\n")
# The model's constraint catalog, for search tests over the real names, groups and descriptions.
catalog = next(e for e in call({"command": "describe", "model": MODEL}) if "constraints" in e)["constraints"]
(Path(__file__).parent / "ess-catalog.json").write_text(json.dumps(catalog, indent=1) + "\n")
print(scored["score"], moved["id"], "->", other["employee"], "| open:", open_shift["id"])
