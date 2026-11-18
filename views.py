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


TIMELINE = """
function drawTimeline(view, mount) {
  try {
    var timeline = SF.rail.createTimeline({
      title: view.title, subtitle: view.subtitle, label: view.label, labelWidth: 220,
      zoomPresets: view.model.axis.endMinute > 7 * 1440 ? ['1w', '2w', '4w', 'reset'] : ['reset'],
      model: view.model
    });
    mount.appendChild(timeline.el);
  } catch (error) {
    mount.textContent = 'The plan could not be drawn: ' + error.message;
  }
}
"""

TIMELINE_GUEST = TIMELINE + """
drawTimeline(JSON.parse(document.getElementById('view-data').textContent), document.getElementById('view'));
"""

# Routes: the map on top, one coloured line per vehicle through its numbered stops,
# unplanned stops grey; a legend to focus one vehicle; the per-vehicle timeline below.
ROUTES_GUEST = TIMELINE + """
(function () {
  var view = JSON.parse(document.getElementById('view-data').textContent);
  var status = document.getElementById('tiles');
  var text = function (tag, value, cls) {
    var el = document.createElement(tag);
    el.textContent = value;
    if (cls) el.className = cls;
    return el;
  };
  document.getElementById('title').textContent = view.title;
  document.getElementById('subtitle').textContent = view.subtitle;
  var ctrl = SF.map.create({container: 'map', center: [0, 0], zoom: 2});
  var map = ctrl.map;
  var bounds = L.latLngBounds([]);
  var describe = function (stop, prefix) {
    var parts = [prefix + stop.label];
    if (stop.arrival) parts.push('arrive ' + stop.arrival.replace('T', ' '));
    if (stop.departure) parts.push('leave ' + stop.departure.replace('T', ' '));
    (stop.broken || []).forEach(function (b) { parts.push('breaks ' + b.constraint); });
    return parts.join(' · ');
  };
  var marker = function (lat, lon, label, color, title) {
    var m = ctrl.addStopNumber({lat: lat, lng: lon, number: label, color: color});
    m.bindPopup(text('span', title));
    m.getElement && m.getElement() && m.getElement().setAttribute('title', title);
    bounds.extend([lat, lon]);
    return m;
  };
  var BROKEN = {hard: '#dc2626', medium: '#e11d48'};
  var legend = document.getElementById('legend');
  var focused = null;
  view.vehicles.forEach(function (route) {
    // The engine draws every route along the roads.
    ctrl.drawEncodedRoute({encoded: route.path || '', color: route.color, weight: 4, opacity: 0.85});
    marker(route.start.lat, route.start.lon, 'S', route.color, route.label + ': ' + route.start.label);
    if (route.end.lat !== route.start.lat || route.end.lon !== route.start.lon) {
      marker(route.end.lat, route.end.lon, 'E', route.color, route.label + ': ' + route.end.label);
    }
    route.stops.forEach(function (stop, n) {
      var worst = (stop.broken || []).filter(function (b) { return !b.shared; })[0];
      marker(stop.lat, stop.lon, String(n + 1), worst ? BROKEN[worst.level] : route.color,
             describe(stop, route.label + ' #' + (n + 1) + ': '));
    });
    var item = document.createElement('button');
    item.type = 'button';
    item.className = 'legend-item';
    var swatch = text('span', '', 'swatch');
    swatch.style.background = route.color;
    item.appendChild(swatch);
    item.appendChild(text('span', route.label, 'name'));
    item.appendChild(text('span', route.stats.join(' · '), 'stats'));
    item.addEventListener('click', function () {
      focused = focused === route.color ? null : route.color;
      if (focused) ctrl.highlight(focused); else ctrl.clearHighlight();
      Array.prototype.forEach.call(legend.children, function (child) {
        child.classList.toggle('dim', Boolean(focused) && child !== item);
      });
    });
    legend.appendChild(item);
  });
  view.unassigned.forEach(function (stop) {
    marker(stop.lat, stop.lon, '•', '#9ca3af', describe(stop, 'Unplanned: '));
  });
  if (view.unassigned.length) {
    legend.appendChild(text('div', view.unassigned.length + ' unplanned', 'unplanned'));
  }
  var fit = function () {
    map.invalidateSize();
    if (bounds.isValid()) map.fitBounds(bounds, {padding: [30, 30], maxZoom: 15, animate: false});
  };
  new ResizeObserver(fit).observe(document.getElementById('map'));
  fit();
  map.eachLayer(function (layer) {
    if (!(layer instanceof L.TileLayer)) return;
    layer.on('tileerror', function () { status.textContent = 'Map tiles unavailable: routes are drawn without the street map.'; });
    layer.on('load', function () { status.textContent = ''; });
  });
  map.attributionControl.setPrefix(false);
  drawTimeline(view.timeline, document.getElementById('view'));
})();
"""

ROUTES_CSS = """
body{font:13px system-ui,sans-serif;color:#1f2937}
header{padding:8px 10px 4px}header h1{margin:0;font-size:14px}header p{margin:2px 0 0;color:#6b7280}
#map{height:420px;border-radius:8px;margin:0 10px}
#tiles{margin:2px 10px;color:#b45309;font-size:12px;min-height:0}
#legend{display:flex;flex-wrap:wrap;gap:6px;padding:6px 10px}
.legend-item{display:inline-flex;align-items:center;gap:6px;border:1px solid #e5e7eb;background:#fff;border-radius:6px;
  padding:3px 8px;cursor:pointer;font:inherit}
.legend-item.dim{opacity:.4}.swatch{width:12px;height:12px;border-radius:3px;display:inline-block}
.legend-item .name{font-weight:600}.legend-item .stats{color:#6b7280}.unplanned{align-self:center;color:#6b7280}
"""


# Assignments: one board per planning variable; a row per value or list owner holding its
# items in order, the unassigned first. A filter narrows every board to the items and rows
# whose words match: each typed word as a substring, or its letters in order inside one
# word or at the starts of later words, so "elc jb" finds "electrical job".
ASSIGNMENTS_GUEST = """
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
        items.forEach(function (item) { list.appendChild(chip(item)); });
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
"""

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
.none{color:#9ca3af}
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
    """A self-contained HTML page drawing an engine view; it reaches nothing but map tiles."""
    base = "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"
    if view.get("kind") == "routes":
        return _page(base + f"; img-src data: {TILES}",
                     _asset("sf.css") + "\n" + _asset("leaflet.css") + "\n" + _asset("sf-map.css") + ROUTES_CSS,
                     "<header><h1 id=\"title\"></h1><p id=\"subtitle\"></p></header><div id=\"map\" "
                     "aria-label=\"Routes on a street map\"></div><p id=\"tiles\" role=\"status\"></p>"
                     "<div id=\"legend\" aria-label=\"Vehicles\"></div><div id=\"view\"></div>",
                     view, [_asset("sf.js"), _asset("leaflet.js"), _asset("sf-map.js")], ROUTES_GUEST)
    if view.get("kind") == "assignments":
        return _page(base, ASSIGNMENTS_CSS,
                     "<header><h1 id=\"title\"></h1><input id=\"filter\" type=\"search\" "
                     "placeholder=\"Filter\" aria-label=\"Filter items\"><span id=\"count\" role=\"status\"></span>"
                     "<p id=\"subtitle\"></p></header><div id=\"view\"></div>",
                     view, [], ASSIGNMENTS_GUEST)
    return _page(base, _asset("sf.css"), "<div id=\"view\"></div>", view, [_asset("sf.js")], TIMELINE_GUEST)
