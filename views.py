"""Plan views as self-contained pages for the Desktop's sandboxed frame.

The engine draws a plan as its model sees it (a timeline, or routes on a map
with a timeline per vehicle) and marks the rules it breaks; this module only
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
    var points = route.path && route.path.length ? route.path
      : [[route.start.lat, route.start.lon]].concat(route.stops.map(function (s) { return [s.lat, s.lon]; }),
                                                    [[route.end.lat, route.end.lon]]);
    ctrl.drawRoute({points: points, color: route.color, weight: 4, opacity: 0.85});
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
    return _page(base, _asset("sf.css"), "<div id=\"view\"></div>", view, [_asset("sf.js")], TIMELINE_GUEST)
