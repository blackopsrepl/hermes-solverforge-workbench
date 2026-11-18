"""Plan views as self-contained pages for the Desktop's sandboxed frame.

The engine draws a plan as its model sees it (a timeline, routes on a map with
a timeline per vehicle, or boards of assignments) and marks the rules it breaks; this module only
wraps that view, with the vendored solverforge-ui and Leaflet code, in an HTML
document that loads nothing but map tiles.
"""

from __future__ import annotations

import json
from functools import lru_cache
from pathlib import Path
from typing import Any

ASSETS = Path(__file__).parent / "assets" / "ui"
TILES = ("https://a.tile.openstreetmap.org https://b.tile.openstreetmap.org "
         "https://c.tile.openstreetmap.org")


@lru_cache(maxsize=None)
def _asset(name: str) -> str:
    return (ASSETS / name).read_text(encoding="utf-8")


# A click on an open item asks the page around the frame to find it a place (fill).
PICK = """
var OPEN_LANES = {open: 1, unplanned: 1, unscheduled: 1, unassigned: 1};
function pick(id, label) {
  try { parent.postMessage({source: 'solverforge-workbench', item: {id: String(id), label: label}}, '*'); } catch (e) {}
}
function pickOpenItems(model, mount) {
  var open = {};
  (model.lanes || []).forEach(function (lane) {
    if (OPEN_LANES[lane.id]) (lane.items || []).forEach(function (item) { open[item.id] = item.label || item.id; });
  });
  mount.addEventListener('click', function (event) {
    var block = event.target.closest && event.target.closest('[data-item-id]');
    if (block && open.hasOwnProperty(block.dataset.itemId)) pick(block.dataset.itemId, open[block.dataset.itemId]);
  });
}
"""

TIMELINE = PICK + """
function drawTimeline(view, mount) {
  try {
    var timeline = SF.rail.createTimeline({
      title: view.title, subtitle: view.subtitle, label: view.label, labelWidth: 220,
      zoomPresets: view.model.axis.endMinute > 7 * 1440 ? ['1w', '2w', '4w', 'reset'] : ['reset'],
      model: view.model
    });
    mount.appendChild(timeline.el);
    pickOpenItems(view.model, mount);
  } catch (error) {
    mount.textContent = 'The plan could not be drawn: ' + error.message;
  }
}
"""

# A page that draws at its own height tells the page around the frame how tall it is, so the
# frame fits it: no empty band below a short plan, no inner scrollbar on a long one.
FIT = """
(function () {
  var last = 0;
  function report() {
    var height = Math.ceil(document.body.getBoundingClientRect().height);
    if (height && height !== last) {
      last = height;
      try { parent.postMessage({source: 'solverforge-workbench', height: height}, '*'); } catch (e) {}
    }
  }
  if (typeof ResizeObserver !== 'undefined') new ResizeObserver(report).observe(document.body);
  report();
})();
"""

TIMELINE_GUEST = TIMELINE + """
drawTimeline(JSON.parse(document.getElementById('view-data').textContent), document.getElementById('view'));
""" + FIT

# Routes: the map alone, filling its frame. One coloured line per vehicle along the roads
# through its numbered stops, broken stops red, unplanned stops grey (a click asks for a
# place). The page around the frame lists the routes and focuses one by posting
# {source, focus: <route id or null>}; the map then dims the others and zooms to it.
ROUTES_MAP_GUEST = PICK + """
(function () {
  var view = JSON.parse(document.getElementById('view-data').textContent);
  var status = document.getElementById('tiles');
  var ctrl = SF.map.create({container: 'map', center: [0, 0], zoom: 2});
  var map = ctrl.map;
  var all = L.latLngBounds([]);
  var byRoute = {};
  var describe = function (stop, prefix) {
    var parts = [prefix + stop.label];
    if (stop.arrival) parts.push('arrive ' + stop.arrival.replace('T', ' '));
    if (stop.departure) parts.push('leave ' + stop.departure.replace('T', ' '));
    (stop.broken || []).forEach(function (b) { parts.push('breaks ' + b.constraint); });
    return parts.join(' · ');
  };
  var marker = function (lat, lon, label, color, title, route) {
    var m = ctrl.addStopNumber({lat: lat, lng: lon, number: label, color: color});
    var tip = document.createElement('span');
    tip.textContent = title;
    m.bindPopup(tip);
    m.getElement && m.getElement() && m.getElement().setAttribute('title', title);
    all.extend([lat, lon]);
    if (route) { route.bounds.extend([lat, lon]); route.markers.push(m); }
    return m;
  };
  var BROKEN = {hard: '#dc2626', medium: '#e11d48'};
  view.vehicles.forEach(function (route) {
    // Colours repeat across many routes, so focus works on each route's own layers.
    var drawn = {bounds: L.latLngBounds([]), markers: [], line: null};
    byRoute[route.id] = drawn;
    drawn.line = ctrl.drawEncodedRoute({encoded: route.path || '', color: route.color, weight: 4, opacity: 0.85});
    marker(route.start.lat, route.start.lon, 'S', route.color, route.label + ': ' + route.start.label, drawn);
    if (route.end.lat !== route.start.lat || route.end.lon !== route.start.lon) {
      marker(route.end.lat, route.end.lon, 'E', route.color, route.label + ': ' + route.end.label, drawn);
    }
    route.stops.forEach(function (stop, n) {
      var worst = (stop.broken || []).filter(function (b) { return !b.shared; })[0];
      marker(stop.lat, stop.lon, String(n + 1), worst ? BROKEN[worst.level] : route.color,
             describe(stop, route.label + ' #' + (n + 1) + ': '), drawn);
    });
  });
  view.unassigned.forEach(function (stop) {
    marker(stop.lat, stop.lon, '•', '#9ca3af', describe(stop, 'Unplanned: ') + ' · click to find it a place')
      .on('click', function () { pick(stop.id, stop.label); });
  });
  var focused = null;
  var fit = function () {
    map.invalidateSize();
    var target = focused && byRoute[focused].bounds.isValid() ? byRoute[focused].bounds : all;
    if (target.isValid()) map.fitBounds(target, {padding: [30, 30], maxZoom: 15, animate: false});
  };
  var paint = function () {
    Object.keys(byRoute).forEach(function (id) {
      var drawn = byRoute[id];
      var on = !focused || focused === id;
      if (drawn.line && drawn.line.setStyle) drawn.line.setStyle(focused === id ? {weight: 6, opacity: 1} : on ? {weight: 4, opacity: 0.85} : {weight: 2, opacity: 0.15});
      if (focused === id && drawn.line && drawn.line.bringToFront) drawn.line.bringToFront();
      drawn.markers.forEach(function (m) { if (m.setOpacity) m.setOpacity(on ? 1 : 0.25); });
    });
  };
  window.addEventListener('message', function (event) {
    var data = event.data;
    if (event.source !== parent || !data || data.source !== 'solverforge-workbench' || !('focus' in data)) return;
    focused = data.focus && byRoute[data.focus] ? data.focus : null;
    paint();
    fit();
  });
  new ResizeObserver(fit).observe(document.getElementById('map'));
  fit();
  map.eachLayer(function (layer) {
    if (!(layer instanceof L.TileLayer)) return;
    layer.on('tileerror', function () { status.textContent = 'Map tiles unavailable: routes are drawn without the street map.'; });
    layer.on('load', function () { status.textContent = ''; });
  });
  map.attributionControl.setPrefix(false);
})();
"""

ROUTES_MAP_CSS = """
html,body{height:100%}
#map{position:absolute;inset:0}
#tiles{position:absolute;left:8px;bottom:8px;z-index:1000;margin:0;padding:2px 6px;border-radius:4px;
  background:rgba(255,255,255,.9);color:#b45309;font:12px system-ui,sans-serif}
#tiles:empty{display:none}
"""


# Assignments: one board per planning variable; a row per value or list owner holding its
# items in order, the unassigned first. A filter narrows every board to the items and rows
# whose words match: each typed word as a substring, or its letters in order inside one
# word or at the starts of later words, so "elc jb" finds "electrical job".
ASSIGNMENTS_GUEST = PICK + """
(function () {
  var view = JSON.parse(document.getElementById('view-data').textContent);
  var text = function (tag, value, cls) {
    var el = document.createElement(tag);
    if (value != null) el.textContent = value;
    if (cls) el.className = cls;
    return el;
  };
  // The Desktop's fuzzyMatch: a substring always matches; otherwise the letters appear in
  // order, the first at a word start, each later one inside the word of the one before it
  // or at the start of a later word.
  var WORD_START = /[\\s\\-_\\/·:(]/;
  var fuzzy = function (needle, hay) {
    var q = Array.from(needle.toLowerCase()), t = Array.from(hay.toLowerCase());
    if (!q.length) return true;
    if (t.join('').indexOf(q.join('')) >= 0) return true;
    var starts = t.map(function (_, j) { return j === 0 || WORD_START.test(t[j - 1]); });
    var sep = t.map(function (ch) { return WORD_START.test(ch); });
    var prev = t.map(function (ch, j) { return ch === q[0] && starts[j]; });
    for (var i = 1; i < q.length; i++) {
      var next = t.map(function () { return false; });
      var lead = false, word = false;
      for (var j = 0; j < t.length; j++) {
        if (starts[j]) word = false;
        if (t[j] === q[i] && !sep[j] && ((j > 0 && prev[j - 1]) || (starts[j] && lead) || word)) next[j] = true;
        if (prev[j]) { lead = true; word = true; }
      }
      prev = next;
    }
    return prev.some(Boolean);
  };
  var matches = function (query, haystack) {
    return query.split(/\\s+/).filter(Boolean).every(function (term) { return fuzzy(term, haystack); });
  };
  var tone = function (broken) {
    var own = (broken || []).filter(function (b) { return !b.shared; });
    if (own.some(function (b) { return b.level === 'hard'; })) return 'hard';
    if (own.some(function (b) { return b.level === 'medium'; })) return 'medium';
    return '';
  };
  var why = function (broken) {
    return (broken || []).map(function (b) { return b.level + ': ' + b.constraint; }).join('\\n');
  };
  document.getElementById('title').textContent = view.title;
  document.getElementById('subtitle').textContent = view.subtitle;
  var mount = document.getElementById('view');
  var filter = document.getElementById('filter');
  var count = document.getElementById('count');
  var chip = function (item) {
    var el = text('span', item.label, 'chip ' + tone(item.broken));
    el.title = item.id + (item.broken && item.broken.length ? '\\n' + why(item.broken) : '');
    return el;
  };
  var draw = function () {
    var query = filter.value.trim();
    var shown = 0, total = 0;
    mount.textContent = '';
    view.boards.forEach(function (board) {
      var section = text('section');
      section.appendChild(text('h2', board.title));
      var rows = [{ label: 'Unassigned', items: board.unassigned, open: true }].concat(board.lanes);
      rows.forEach(function (lane) {
        if (lane.open && !lane.items.length) return;
        total += lane.items.length;
        var laneHit = !query || (!lane.open && matches(query, lane.label));
        var items = laneHit ? lane.items : lane.items.filter(function (item) { return matches(query, item.label + ' ' + item.id); });
        if (query && !laneHit && !items.length) return;
        shown += items.length;
        var row = text('div', null, 'row' + (lane.open ? ' open' : ''));
        var head = text('div', lane.label, 'lane ' + tone(lane.broken));
        if (lane.broken && lane.broken.length) head.title = why(lane.broken);
        head.appendChild(text('span', String(lane.items.length), 'n'));
        row.appendChild(head);
        var list = text('div', null, 'items');
        if (!items.length) list.appendChild(text('span', 'Nothing assigned', 'none'));
        items.forEach(function (item) {
          var el = chip(item);
          if (lane.open) {
            el.classList.add('pick');
            el.title += '\\nClick to find it a place';
            el.addEventListener('click', function () { pick(item.id, item.label); });
          }
          list.appendChild(el);
        });
        row.appendChild(list);
        section.appendChild(row);
      });
      mount.appendChild(section);
    });
    count.textContent = query ? shown + ' of ' + total : '';
  };
  filter.addEventListener('input', draw);
  filter.addEventListener('keydown', function (event) { if (event.key === 'Escape') { filter.value = ''; draw(); } });
  draw();
})();
""" + FIT

ASSIGNMENTS_CSS = """
body{font:13px system-ui,sans-serif;color:#1f2937}
header{display:flex;flex-wrap:wrap;align-items:baseline;gap:4px 12px;padding:8px 10px 4px}
header h1{margin:0;font-size:14px}header p{margin:0;color:#6b7280;flex-basis:100%}
#filter{margin-left:auto;font:inherit;padding:3px 8px;border:1px solid #d1d5db;border-radius:6px;min-width:200px}
#count{color:#6b7280}
section{padding:4px 10px 8px}h2{font-size:13px;margin:8px 0 4px}
.row{display:grid;grid-template-columns:200px 1fr;gap:8px;padding:4px 0;border-top:1px solid #f3f4f6}
.lane{font-weight:600;overflow-wrap:anywhere}.lane .n{font-weight:400;color:#9ca3af;margin-left:6px}
.row.open .lane{color:#6b7280}
.items{display:flex;flex-wrap:wrap;gap:4px}
.chip{border:1px solid #e5e7eb;background:#f9fafb;border-radius:5px;padding:1px 7px}
.hard{color:#b91c1c}.chip.hard{background:#fee2e2;border-color:#fca5a5}
.medium{color:#be123c}.chip.medium{background:#ffe4e6;border-color:#fda4af}
.none{color:#9ca3af}.chip.pick{cursor:pointer}.chip.pick:hover{border-color:#6b7280}
"""


def _payload(view: dict[str, Any]) -> str:
    # JSON inside a script element: only "</" (and HTML comment openers) could end it early.
    return json.dumps(view, ensure_ascii=False).replace("</", "<\\/").replace("<!--", "<\\!--")


def _page(csp: str, css: str, body: str, view: dict[str, Any], scripts: list[str], guest: str) -> str:
    return (
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"referrer\" content=\"no-referrer\">"
        f"<meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">"
        "<style>" + css + "\nhtml,body{margin:0;background:transparent}#view{padding:0}</style></head>"
        "<body>" + body + "<script id=\"view-data\" type=\"application/json\">" + _payload(view) + "</script>"
        + "".join("<script>" + script + "\n</script>" for script in scripts)
        + "<script>" + guest + "</script></body></html>"
    )


def document(view: dict[str, Any]) -> str:
    """A self-contained HTML page drawing an engine view; it reaches nothing but map tiles.
    A routes view draws as its map alone; the page lists the routes beside it (see routes)."""
    base = "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"
    if view.get("kind") == "routes":
        return _page(base + f"; img-src data: {TILES}",
                     _asset("sf.css") + "\n" + _asset("leaflet.css") + "\n" + _asset("sf-map.css") + ROUTES_MAP_CSS,
                     "<div id=\"map\" aria-label=\"Routes on a street map\"></div><p id=\"tiles\" role=\"status\"></p>",
                     view, [_asset("sf.js"), _asset("leaflet.js"), _asset("sf-map.js")], ROUTES_MAP_GUEST)
    if view.get("kind") == "assignments":
        return _page(base, ASSIGNMENTS_CSS,
                     "<header><h1 id=\"title\"></h1><input id=\"filter\" type=\"search\" "
                     "placeholder=\"Filter\" aria-label=\"Filter items\"><span id=\"count\" role=\"status\"></span>"
                     "<p id=\"subtitle\"></p></header><div id=\"view\"></div>",
                     view, [], ASSIGNMENTS_GUEST)
    return _page(base, _asset("sf.css"), "<div id=\"view\"></div>", view, [_asset("sf.js")], TIMELINE_GUEST)


def drawn(view: dict[str, Any]) -> dict[str, Any]:
    """What the page needs to show a view. A routes view comes as three parts: the map
    (a page of its own that fills its frame), the per-vehicle timeline (another page), and
    the routes and unplanned stops as data the page lists itself."""
    if view.get("kind") != "routes":
        return {"kind": view.get("kind"), "document": document(view)}
    timeline = view.get("timeline") or {}
    lanes = len(((timeline.get("model") or {}).get("lanes")) or [])
    return {
        "kind": "routes", "title": view.get("title", ""), "subtitle": view.get("subtitle", ""),
        "map": document(view),
        "timeline": document({"kind": "timeline", **timeline}) if timeline else None,
        "lanes": lanes,
        "routes": [{"id": r["id"], "label": r["label"], "color": r["color"], "stats": r.get("stats", []),
                    "stops": len(r.get("stops", [])),
                    "broken": {level: sum(1 for s in r.get("stops", []) for b in (s.get("broken") or [])
                                          if not b.get("shared") and b.get("level") == level)
                               for level in ("hard", "medium")}}
                   for r in view.get("vehicles", [])],
        "unassigned": [{"id": u["id"], "label": u["label"]} for u in view.get("unassigned", [])],
    }
