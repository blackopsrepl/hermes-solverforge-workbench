"""Regenerate the plan-view fixtures: views exactly as the stock engine draws them.

Run from the repository root after `make engine` (and after make_roster_fixture.py
when the run fixture changes):

    uv run --no-project --python 3.12 python tests/fixtures/make_view_fixtures.py

- ess-month-view.json: the roster of the stored month run, with its
  broken hard rule and its open shift.
- routes-view.json: the first routing model's demo, solved for a few
  seconds, as routes on a map with a timeline per vehicle.
"""

import json
from pathlib import Path

from engine_call import call, models

HERE = Path(__file__).parent
run = json.loads((HERE / "ess-month-run.json").read_text())
roster = next(e for e in call({"command": "view", "model": run["model_id"], "plan": run["plan"]}) if "view" in e)
(HERE / "ess-month-view.json").write_text(json.dumps(roster["view"], indent=1, ensure_ascii=False) + "\n")

routing = []
for listed in models():
    described = next(e for e in call({"command": "describe", "model": listed["id"]}) if "constraints" in e)
    if described.get("view") == "routes":
        routing.append((listed["id"], described["demos"][0]))
model, demo_id = routing[0]
demo = next(e for e in call({"command": "demo", "model": model, "demo": demo_id}) if "plan" in e)
done = next(e for e in call({"command": "solve", "model": model, "plan": demo["plan"],
                             "termination": {"seconds": 5}}) if e["event"] == "done")
routes = next(e for e in call({"command": "view", "model": model, "plan": done["plan"]}) if "view" in e)
(HERE / "routes-view.json").write_text(json.dumps({"model_id": model, "demo": demo_id, "score": done["score"],
                                                   "view": routes["view"]}, indent=1, ensure_ascii=False) + "\n")
print("roster lanes:", len(roster["view"]["model"]["lanes"]), "| routes:", model, demo_id, done["score"],
      len(routes["view"]["vehicles"]), "vehicles")
