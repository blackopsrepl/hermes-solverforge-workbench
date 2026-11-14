"""Plan views: a model's plan drawn as a schedule.

A view turns a plan (in the model's own dataset format) and the run's analysis
into the numeric model the solverforge-ui scheduling timeline draws, and wraps
it in a self-contained HTML document the Desktop shows in a sandboxed frame.
Models without a view get none; every number still comes from the plan.
"""

from __future__ import annotations

import json
import re
from datetime import datetime, timedelta
from functools import lru_cache
from pathlib import Path
from typing import Any, Callable

ASSETS = Path(__file__).parent / "assets" / "ui"
DAY = 1440
WEEKDAYS = ("monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday")
# Tones for the first tag of a shift; other tags take the remaining tones in order of appearance.
TAG_TONES = {"early": "amber", "day": "amber", "morning": "amber", "late": "blue", "evening": "blue",
             "night": "violet"}
SPARE_TONES = ("cyan", "emerald", "slate", "blue", "amber", "violet")
# Overlay colours for time an employee cannot or would rather not work.
UNAVAILABLE = {"background": "rgba(75, 85, 99, 0.16)", "border": "#6b7280", "overlay": "rgba(75, 85, 99, 0.16)",
               "text": "#374151"}
RATHER_NOT = {"background": "rgba(156, 163, 175, 0.12)", "border": "#9ca3af", "overlay": "rgba(156, 163, 175, 0.12)",
              "text": "#4b5563"}
MAX_FLAGGED_MATCHES = 100
# Lane badges for rules an employee's roster breaks as a whole.
BROKEN_BADGE = {
    "hard": {"bg": "rgba(220, 38, 38, 0.12)", "border": "1px solid #dc2626", "color": "#b91c1c"},
    "medium": {"bg": "rgba(225, 29, 72, 0.08)", "border": "1px solid #fb7185", "color": "#be123c"},
}
MAX_LANE_RULES = 2
MAX_LANE_BADGES = 4


class ViewError(ValueError):
    """The plan cannot be drawn: a field the view needs is missing or malformed."""


def _minutes(stamp: Any, start: datetime, what: str) -> int:
    try:
        moment = datetime.fromisoformat(str(stamp))
    except ValueError as exc:
        raise ViewError(f"{what} is not a local date-time: {stamp!r}") from exc
    return int((moment - start) / timedelta(minutes=1))


def _hhmm(minute: int, start: datetime) -> str:
    return (start + timedelta(minutes=minute)).strftime("%H:%M")


Flags = dict[str, list[tuple[str, str]]]
SCORE_PART = re.compile(r"(-?\d+)(hard|medium|soft)")


def _cost(score: Any, level: str) -> int:
    """How much a match costs at its level, as a positive number."""
    parts = {name: int(value) for value, name in SCORE_PART.findall(str(score))}
    return -parts.get(level, 0)


def _flags(analysis: list[dict[str, Any]] | None) -> tuple[Flags, dict[str, list[tuple[str, str, int]]]]:
    """(shift id, employee id) -> [(level, constraint)] for every hard or medium match that costs score.

    A match justified by an employee comes from a rule over that employee's
    whole roster: it names every shift they work, so it marks the employee,
    not each shift. A match without an employee marks the shifts it names.
    """
    shifts: Flags = {}
    employees: dict[str, list[tuple[str, str, int]]] = {}
    for item in analysis or []:
        if item.get("level") not in ("hard", "medium"):
            continue
        for match in (item.get("matches") or [])[:MAX_FLAGGED_MATCHES]:
            if not str(match.get("score", "")).startswith("-") and "/-" not in str(match.get("score", "")):
                continue
            entities = match.get("entities") or []
            named = [str(e["id"]) for e in entities if e.get("type") == "Employee" and e.get("id")]
            for employee_id in named:
                employees.setdefault(employee_id, []).append(
                    (item["level"], item["name"], _cost(match.get("score"), item["level"])))
            if named:
                continue
            for entity in entities:
                if entity.get("type") == "Shift" and entity.get("id"):
                    shifts.setdefault(str(entity["id"]), []).append((item["level"], item["name"]))
    return shifts, employees


def _rule_badges(broken: list[tuple[str, str, int]]) -> list[dict[str, Any]]:
    """Badges naming the rules an employee's roster breaks: hard ones first, then the costliest."""
    level_of: dict[str, str] = {}
    cost_of: dict[str, int] = {}
    for level, name, cost in broken:
        level_of[name] = level
        cost_of[name] = cost_of.get(name, 0) + cost
    names = sorted(level_of, key=lambda name: (level_of[name] != "hard", -cost_of[name], name))
    badges = [{"label": name, "style": BROKEN_BADGE[level_of[name]]} for name in names[:MAX_LANE_RULES]]
    if len(names) > MAX_LANE_RULES:
        rest = names[MAX_LANE_RULES:]
        worst = "hard" if any(level_of[name] == "hard" for name in rest) else "medium"
        badges.append({"label": f"+{len(rest)} more", "style": BROKEN_BADGE[worst]})
    return badges


def roster(plan: dict[str, Any], analysis: list[dict[str, Any]] | None = None) -> dict[str, Any]:
    """Employee scheduling: one lane per employee, open shifts on top, time off as overlays."""
    schedule = plan.get("schedule") or {}
    try:
        start = datetime.fromisoformat(str(schedule["start"]))
        end_at = datetime.fromisoformat(str(schedule["end"]))
    except (KeyError, ValueError) as exc:
        raise ViewError("the plan's schedule needs a start and an end date-time") from exc
    day0 = start.replace(hour=0, minute=0, second=0, microsecond=0)
    span_days = max(1, -(-int((end_at - day0) / timedelta(minutes=1)) // DAY))
    weekend = {str(d).lower() for d in schedule.get("weekend") or ("saturday", "sunday")}
    flags, employee_flags = _flags(analysis)

    tones: dict[str, str] = dict(TAG_TONES)
    def tone_of(tags: list[str]) -> str:
        key = (tags or ["shift"])[0]
        if key not in tones:
            tones[key] = SPARE_TONES[(len(tones) - len(TAG_TONES)) % len(SPARE_TONES)]
        return tones[key]

    def item(shift: dict[str, Any]) -> dict[str, Any]:
        begin = _minutes(shift.get("start"), day0, f"shift {shift.get('id')} start")
        finish = _minutes(shift.get("end"), day0, f"shift {shift.get('id')} end")
        if finish <= begin:
            raise ViewError(f"shift {shift.get('id')} ends before it starts")
        tags = [str(t) for t in shift.get("tags") or []]
        broken = flags.get(str(shift.get("id")), [])
        notes = [f"{_hhmm(begin, day0)}–{_hhmm(finish, day0)}"]
        if tags:
            notes.append(", ".join(tags))
        if shift.get("pinned"):
            notes.append("pinned")
        if shift.get("optional"):
            notes.append("optional")
        if broken:
            notes.append("breaks: " + "; ".join(sorted({name for _, name in broken})))
        return {
            "id": str(shift.get("id")),
            "startMinute": begin,
            "endMinute": finish,
            "label": f"{'📌 ' if shift.get('pinned') else ''}{tags[0] if tags else 'shift'}",
            "meta": " · ".join(notes),
            "tone": "red" if any(level == "hard" for level, _ in broken) else "rose" if broken else tone_of(tags),
        }

    def overlays(employee: dict[str, Any]) -> list[dict[str, Any]]:
        out = []
        # Time off is grey, so red and rose on the roster only ever mean a broken rule.
        for key, label, tone in (("unavailable", "Unavailable", UNAVAILABLE), ("unwantedTimes", "Rather not", RATHER_NOT),
                                 ("preferredTimes", "Preferred", "emerald")):
            for span in employee.get(key) or []:
                begin = _minutes(span.get("from"), day0, f"{employee.get('id')} {key}")
                finish = _minutes(span.get("to"), day0, f"{employee.get('id')} {key}")
                if finish > begin:
                    out.append({"startMinute": begin, "endMinute": finish, "label": label, "tone": tone})
        return out

    shifts = [s for s in plan.get("shifts") or [] if isinstance(s, dict)]
    by_employee: dict[str, list[dict[str, Any]]] = {}
    open_shifts = []
    for shift in shifts:
        if shift.get("employee"):
            by_employee.setdefault(str(shift["employee"]), []).append(shift)
        elif not shift.get("alternativeOf"):  # an unchosen alternative is not an open shift
            open_shifts.append(shift)

    lanes = []
    if open_shifts:
        mandatory = sum(1 for s in open_shifts if not s.get("optional"))
        lanes.append({
            "id": "open", "label": "Open shifts", "mode": "detailed",
            "badges": [{"label": f"{mandatory} mandatory"}] if mandatory else [],
            "stats": [{"label": "Shifts", "value": len(open_shifts)}],
            "items": [item(s) for s in sorted(open_shifts, key=lambda s: str(s.get("start")))],
        })
    for employee in plan.get("employees") or []:
        employee_id = str(employee.get("id"))
        mine = sorted(by_employee.get(employee_id, []), key=lambda s: str(s.get("start")))
        items = [item(s) for s in mine]
        minutes = sum(i["endMinute"] - i["startMinute"] for i in items)
        lanes.append({
            "id": f"employee:{employee_id}",
            "label": str(employee.get("name") or employee_id),
            "mode": "detailed",
            "badges": (_rule_badges(employee_flags.get(employee_id, [])) + [
                {"label": str(s if isinstance(s, str) else s.get("id"))} for s in (employee.get("skills") or [])
            ])[:MAX_LANE_BADGES],
            "stats": [{"label": "Shifts", "value": len(items)}, {"label": "Hours", "value": round(minutes / 60, 1)}],
            "overlays": overlays(employee),
            "items": items,
        })

    days = []
    for n in range(span_days):
        date = day0 + timedelta(days=n)
        days.append({"label": date.strftime("%a"), "subLabel": date.strftime("%-d %b"),
                     "isWeekend": WEEKDAYS[date.weekday()] in weekend})
    ticks = [{"minute": m, "label": f"{(m % DAY) // 60:02d}:00"} for m in range(0, span_days * DAY + 1, 360)]
    shown = min(span_days, 7 if span_days > 14 else span_days)
    return {
        "title": "Roster",
        "subtitle": f"{len(shifts)} shifts · {len(plan.get('employees') or [])} employees · "
                    f"{len(open_shifts)} open · red: breaks a hard rule, on a shift or an employee's row · "
                    "rose: a medium one · grey: time off",
        "label": "Employee",
        "model": {
            "axis": {"startMinute": 0, "endMinute": span_days * DAY, "days": days, "ticks": ticks,
                     "initialViewport": {"startMinute": 0, "endMinute": shown * DAY}},
            "lanes": lanes,
        },
    }


VIEWS: dict[str, Callable[[dict[str, Any], list[dict[str, Any]] | None], dict[str, Any]]] = {
    "employee-scheduling": roster,
}


def has_view(model_id: str) -> bool:
    return model_id in VIEWS


def timeline(model_id: str, plan: dict[str, Any], analysis: list[dict[str, Any]] | None = None) -> dict[str, Any]:
    builder = VIEWS.get(model_id)
    if builder is None:
        raise ViewError(f"model {model_id} has no schedule view")
    return builder(plan, analysis)


@lru_cache(maxsize=1)
def _assets() -> tuple[str, str]:
    return (ASSETS / "sf.css").read_text(encoding="utf-8"), (ASSETS / "sf.js").read_text(encoding="utf-8")


GUEST = """
(function () {
  var data = JSON.parse(document.getElementById('view-data').textContent);
  var mount = document.getElementById('view');
  try {
    var view = SF.rail.createTimeline({
      title: data.title, subtitle: data.subtitle, label: data.label, labelWidth: 220,
      zoomPresets: data.model.axis.endMinute > 7 * 1440 ? ['1w', '2w', '4w', 'reset'] : ['reset'],
      model: data.model
    });
    mount.appendChild(view.el);
  } catch (error) {
    mount.textContent = 'The schedule could not be drawn: ' + error.message;
  }
})();
"""


def document(view: dict[str, Any]) -> str:
    """A self-contained HTML page drawing ``view``; no network access, no host access."""
    css, js = _assets()
    # JSON inside a script element: only "</" could end it early.
    payload = json.dumps(view, ensure_ascii=False).replace("</", "<\\/")
    return (
        "<!doctype html><html><head><meta charset=\"utf-8\">"
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; "
        "script-src 'unsafe-inline'\">"
        "<style>" + css + "\nhtml,body{margin:0;background:transparent}#view{padding:0}</style></head>"
        "<body><div id=\"view\"></div><script id=\"view-data\" type=\"application/json\">" + payload + "</script>"
        "<script>" + js + "\n</script><script>" + GUEST + "</script></body></html>"
    )
