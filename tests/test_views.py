"""The roster view on the engine's own output (tests/fixtures/ess-small-week-run.json)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from solverforge_workbench_plugin import views

FIXTURE = json.loads((Path(__file__).parent / "fixtures" / "ess-small-week-run.json").read_text())


def lanes(view: dict) -> dict[str, dict]:
    return {lane["id"]: lane for lane in view["model"]["lanes"]}


def test_one_lane_per_employee_and_open_shifts_on_top():
    view = views.timeline("employee-scheduling", FIXTURE["plan"], FIXTURE["analysis"])
    found = view["model"]["lanes"]
    assert found[0]["id"] == "open"
    assert [i["id"] for i in found[0]["items"]] == [FIXTURE["opened"]]
    assert len(found) == 1 + len(FIXTURE["plan"]["employees"])
    placed = sum(len(lane["items"]) for lane in found)
    assert placed == len(FIXTURE["plan"]["shifts"])


def test_times_are_integer_minutes_from_the_first_day():
    view = views.timeline("employee-scheduling", FIXTURE["plan"], FIXTURE["analysis"])
    axis = view["model"]["axis"]
    assert axis["startMinute"] == 0 and axis["endMinute"] == 7 * 1440
    assert [d["isWeekend"] for d in axis["days"]] == [False] * 5 + [True] * 2
    shift = next(s for s in FIXTURE["plan"]["shifts"] if s["start"] == "2027-03-01T07:00")
    item = next(i for lane in view["model"]["lanes"] for i in lane["items"] if i["id"] == shift["id"])
    assert (item["startMinute"], item["endMinute"]) == (7 * 60, 15 * 60)
    every = [i for lane in view["model"]["lanes"] for i in lane["items"]]
    assert all(isinstance(i["startMinute"], int) and isinstance(i["endMinute"], int) for i in every)


def test_a_shift_the_engine_names_in_a_costly_match_is_marked_with_the_rule():
    view = views.timeline("employee-scheduling", FIXTURE["plan"], FIXTURE["analysis"])
    item = lanes(view)["open"]["items"][0]
    assert item["tone"] == "rose"
    assert "breaks: Mandatory shift left open" in item["meta"]


def test_a_rule_over_an_employees_whole_roster_marks_their_row_not_every_shift():
    # The engine justifies a per-employee rule by the employee and every shift
    # they work; only two of Amina's shifts overlap.
    view = views.timeline("employee-scheduling", FIXTURE["plan"], FIXTURE["analysis"])
    amina = lanes(view)["employee:amina"]
    # Her two costliest broken rules (-480 each) by name, then a count for the
    # cheaper one (-1).
    assert [b["label"] for b in amina["badges"]][:3] == [
        "Minutes per period outside required range", "Overlapping shifts", "+1 more"]
    assert amina["badges"][0]["style"] == views.BROKEN_BADGE["hard"]
    assert len(amina["badges"]) <= views.MAX_LANE_BADGES
    assert all("Overlapping shifts" not in item["meta"] for item in amina["items"])
    assert all(item["tone"] != "red" for item in amina["items"])


def test_time_off_and_preferences_are_drawn_behind_the_shifts():
    view = views.timeline("employee-scheduling", FIXTURE["plan"], FIXTURE["analysis"])
    amina = lanes(view)["employee:amina"]
    unavailable = next(o for o in amina["overlays"] if o["label"] == "Unavailable")
    assert (unavailable["startMinute"], unavailable["endMinute"]) == (2 * 1440, 3 * 1440)
    # Red is reserved for broken rules; time off is drawn in grey.
    assert all(o["tone"] not in ("red", "rose") for o in amina["overlays"])
    assert amina["stats"][0]["label"] == "Shifts"


def test_a_plan_without_a_schedule_is_refused_with_the_reason():
    with pytest.raises(views.ViewError, match="schedule needs a start and an end"):
        views.timeline("employee-scheduling", {"shifts": []})
    with pytest.raises(views.ViewError, match="has no schedule view"):
        views.timeline("toy", {})


def test_the_document_is_self_contained_and_cannot_be_broken_out_of():
    plan = json.loads(json.dumps(FIXTURE["plan"]))
    plan["employees"][0]["name"] = "</script><script>alert(1)</script>"
    page = views.document(views.timeline("employee-scheduling", plan))
    assert "default-src 'none'" in page
    assert "</script><script>alert(1)" not in page
    assert "SF.rail.createTimeline" in page
    assert "http://" not in page.split("<script id=\"view-data\"")[1].split("</script>")[0]
