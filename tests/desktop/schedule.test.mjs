// The pages the backend serves, run in a DOM: solverforge-ui (and Leaflet) must accept the views and draw them.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { JSDOM, VirtualConsole } from 'jsdom'

const here = dirname(fileURLToPath(import.meta.url))
const { document: page } = JSON.parse(readFileSync(join(here, 'fixtures', 'view.json'), 'utf8'))
const run = JSON.parse(readFileSync(join(here, '..', 'fixtures', 'ess-month-run.json'), 'utf8'))

function draw(html) {
  const errors = []
  const virtualConsole = new VirtualConsole()
  virtualConsole.on('jsdomError', error => errors.push(error.message))
  // jsdom has no layout, so no ResizeObserver; browsers all do.
  const beforeParse = window => {
    window.ResizeObserver = class { observe() {} unobserve() {} disconnect() {} }
  }
  const dom = new JSDOM(html, { runScripts: 'dangerously', pretendToBeVisual: true, virtualConsole, beforeParse })
  return { doc: dom.window.document, errors }
}

test('the roster draws one lane per employee plus the open shifts', () => {
  const { doc, errors } = draw(page)
  assert.deepEqual(errors, [])
  const root = doc.querySelector('#view')
  assert.doesNotMatch(root.textContent, /could not be drawn/)
  assert.ok(root.querySelector('.sf-rail-timeline'))
  const text = root.textContent
  assert.match(text, /Roster/)
  assert.match(text, /Open shifts/)
  for (const employee of run.plan.employees) assert.match(text, new RegExp(employee.name))
})

test('a model the timeline rejects shows the reason instead of a blank frame', () => {
  const broken = page.replace('{"endMinute": 10080, "startMinute": 0}', '{"endMinute": 10080, "startMinute": 0.5}')
  assert.notEqual(broken, page)
  const { doc } = draw(broken)
  assert.match(doc.querySelector('#view').textContent, /The plan could not be drawn/)
})

const routesPage = JSON.parse(readFileSync(join(here, 'pages', 'routes.json'), 'utf8')).document
const routes = JSON.parse(readFileSync(join(here, '..', 'fixtures', 'routes-view.json'), 'utf8')).view

test('routes draw a line per vehicle through numbered stops, a legend, and the per-vehicle timeline', () => {
  const { doc, errors } = draw(routesPage)
  // jsdom loads no images: tile errors are expected, script errors are not.
  assert.deepEqual(errors.filter(m => !/Could not load img/.test(m)), [])
  assert.equal(doc.querySelector('#title').textContent, routes.title)
  const legend = [...doc.querySelectorAll('#legend .legend-item .name')].map(n => n.textContent)
  assert.deepEqual(legend, routes.vehicles.map(v => v.label))
  const stops = routes.vehicles.reduce((n, v) => n + v.stops.length, 0) + routes.unassigned.length
  // One marker per stop, plus each vehicle's start (and end, when it differs).
  const markers = doc.querySelectorAll('.sf-marker-stop').length
  assert.ok(markers >= stops + routes.vehicles.length, `${markers} markers for ${stops} stops`)
  assert.equal(doc.querySelectorAll('path.leaflet-interactive').length, routes.vehicles.length)
  assert.ok(doc.querySelector('#view .sf-rail-timeline'))
  assert.doesNotMatch(doc.querySelector('#view').textContent, /could not be drawn/)
})
