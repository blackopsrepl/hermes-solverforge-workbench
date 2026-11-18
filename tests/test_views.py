"""Plan-view pages on views the engine really draws (tests/fixtures/*-view.json)."""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

import pytest

from solverforge_workbench_plugin import views

FIXTURES = Path(__file__).parent / "fixtures"
ROSTER = json.loads((FIXTURES / "ess-month-view.json").read_text())
RUN = json.loads((FIXTURES / "ess-month-run.json").read_text())
ROUTES = json.loads((FIXTURES / "routes-view.json").read_text())


def lanes(view: dict) -> dict[str, dict]:
    return {lane["id"]: lane for lane in view["model"]["lanes"]}


def test_the_engine_draws_the_roster_with_open_shifts_on_top_and_an_employees_broken_rules_on_their_row():
    assert ROSTER["kind"] == "timeline"
    found = lanes(ROSTER)
    assert ROSTER["model"]["lanes"][0]["id"] == "open"
    assert RUN["opened"] in [i["id"] for i in found["open"]["items"]]
    assert len(found) == 1 + len(RUN["plan"]["employees"])
    # The overlap is a rule over the employee's roster: a red badge on the row, no red shift.
    moved_to = next(s["employee"] for s in RUN["plan"]["shifts"] if s["id"] == RUN["moved"])
    row = found[f"employee:{moved_to}"]
    assert row["badges"][0]["label"] == "Overlapping shifts"
    assert row["badges"][0]["style"]["color"] == "#b91c1c"
    assert all(item["tone"] != "red" for item in row["items"])
    assert all(o["tone"] not in ("red", "rose") for lane in found.values() for o in lane.get("overlays", []))


def test_a_timeline_page_is_self_contained_and_cannot_be_broken_out_of():
    view = json.loads(json.dumps(ROSTER))
    view["model"]["lanes"][1]["label"] = "</script><script>alert(1)</script>"
    page = views.document(view)
    assert "default-src 'none'" in page and "img-src" not in page
    assert "</script><script>alert(1)" not in page
    assert "SF.rail.createTimeline" in page
    data = page.split("<script id=\"view-data\" type=\"application/json\">")[1].split("</script>")[0]
    assert json.loads(data.replace("<\\/", "</"))["model"]["lanes"][1]["label"].startswith("</script>")


def test_the_engine_draws_routes_with_every_stop_on_a_vehicle_or_left_unplanned():
    view = ROUTES["view"]
    assert view["kind"] == "routes"
    assert view["vehicles"] and all(v["color"].startswith("#") for v in view["vehicles"])
    for vehicle in view["vehicles"]:
        for point in [vehicle["start"], vehicle["end"], *vehicle["stops"]]:
            assert -90 <= point["lat"] <= 90 and -180 <= point["lon"] <= 180
    stops = [s["id"] for v in view["vehicles"] for s in v["stops"]] + [s["id"] for s in view["unassigned"]]
    assert len(stops) == len(set(stops))
    assert view["timeline"]["model"]["lanes"]
    # Every route is drawn along the roads: an encoded polyline from start to end.
    assert all(isinstance(v.get("path"), str) and len(v["path"]) > 20 for v in view["vehicles"] if v["stops"])


def test_a_routes_page_loads_only_street_tiles_and_draws_the_map_and_the_timeline():
    page = views.document(ROUTES["view"])
    csp = page.split("Content-Security-Policy\" content=\"")[1].split("\"")[0]
    assert "default-src 'none'" in csp and "connect-src" not in csp
    assert csp.split("img-src ")[1].split(";")[0] == "data: " + views.TILES
    assert "SF.map.create" in page and "drawTimeline(view.timeline" in page
    assert "Leaflet 1.9.4" in page


ASSIGNMENTS = {"kind": "assignments", "title": "Crew tasks", "subtitle": "1 unassigned",
               "boards": [{"title": "Task by Resource", "item": "Task", "lane": "Resource",
                           "lanes": [{"id": "r-0", "label": "Ada", "items": [
                               {"id": "t-1", "label": "plumbing job 1",
                                "broken": [{"level": "hard", "constraint": "skill_match", "cost": 1,
                                            "shared": False}]}]}],
                           "unassigned": [{"id": "t-0", "label": "electrical job 0"}]}]}


def test_an_assignments_page_is_self_contained_and_draws_from_its_data():
    page = views.document(ASSIGNMENTS)
    assert "default-src 'none'" in page and "img-src" not in page
    assert "<script src" not in page and "https://" not in page
    assert '"skill_match"' in page and 'id="filter"' in page


@pytest.mark.skipif(shutil.which("node") is None, reason="needs Node")
def test_the_assignments_filter_matches_like_the_desktop_search():
    guest = views.document(ASSIGNMENTS).split("<script>")[-1].split("</script>")[0]
    matcher = guest[guest.index("var WORD_START"):guest.index("var tone")]
    cases = [["rst btwn", "Rest between shifts", True], ["cdo", "Consecutive days off", True],
             ["fair", "Rest between shifts", False], ["elc jb", "electrical job 4", True],
             ["ecl", "electrical", True], ["ti", "Rest between", False], ["xyz", "electrical job", False]]
    script = matcher + f"console.log(JSON.stringify({json.dumps(cases)}.map(([q, h]) => matches(q, h))))"
    found = json.loads(subprocess.run(["node", "-e", script], capture_output=True, text=True, check=True).stdout)
    assert found == [want for _, _, want in cases]
