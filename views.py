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
# A shift named in an entity text of an analysis match, as the engine prints it.
SHIFT_IN_MATCH = re.compile(r'Shift \{ index: \d+, id: "((?:[^"\\]|\\.)*)"')
MAX_FLAGGED_MATCHES = 100


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


def _flags(analysis: list[dict[str, Any]] | None) -> dict[str, list[tuple[str, str]]]:
    """Shift id -> [(level, constraint)] for every hard or medium match that costs score."""
    flags: dict[str, list[tuple[str, str]]] = {}
    for item in analysis or []:
        if item.get("level") not in ("hard", "medium"):
            continue
        for match in (item.get("matches") or [])[:MAX_FLAGGED_MATCHES]:
            if not str(match.get("score", "")).startswith("-") and "/-" not in str(match.get("score", "")):
                continue
            for entity in match.get("entities") or []:
                for shift_id in SHIFT_IN_MATCH.findall(entity.get("text", "")):
                    flags.setdefault(shift_id, []).append((item["level"], item["name"]))
    return flags


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
    flags = _flags(analysis)

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
        for key, label, tone in (("unavailable", "Unavailable", "red"), ("unwantedTimes", "Rather not", "amber"),
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
            "badges": [{"label": str(s if isinstance(s, str) else s.get("id"))} for s in (employee.get("skills") or [])][:3],
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
                    f"{len(open_shifts)} open · red: breaks a hard rule",
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
