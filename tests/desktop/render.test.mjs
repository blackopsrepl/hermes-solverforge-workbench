// The Desktop half rendered with React's server renderer, against real tool answers
// (fixtures/ is written by make_fixtures.py from the plugin and the testkit engine).
import assert from 'node:assert/strict'
import { cpSync, mkdirSync, readdirSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { navigated, opened, setFixture } from '@hermes/plugin-sdk'
import { createElement as h } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'

// plugin.js imports bare specifiers; a copy beside node_modules resolves them here.
const here = dirname(fileURLToPath(import.meta.url))
mkdirSync(join(here, '.build'), { recursive: true })
cpSync(join(here, '../../desktop/plugin.js'), join(here, '.build/plugin.mjs'))
const { default: plugin, Workbench, RunView, ScoreChart, StatusChip, parseScore, openBeside, openPage } = await import('./.build/plugin.mjs')

const real = Object.fromEntries(
  readdirSync(join(here, 'fixtures')).map(f => [f.replace('.json', ''), JSON.parse(readFileSync(join(here, 'fixtures', f), 'utf8'))])
)
const items = []
plugin.register({ rest: async () => ({}), onEvent() {}, registerMany: xs => items.push(...xs) })
const text = html => html.replace(/<[^>]+>/g, ' ').replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, '&').replace(/\s+/g, ' ').trim()
const render = node => renderToStaticMarkup(node)

test('scores parse into levels, including medium and decimals', () => {
  assert.deepEqual(parseScore('-6hard/0soft'), [{ level: 'hard', value: -6 }, { level: 'soft', value: 0 }])
  assert.deepEqual(parseScore('0hard/-2medium/-1.5soft').map(p => p.value), [0, -2, -1.5])
  assert.deepEqual(parseScore(null), [])
})

test('the catalog lists each model with its constraints by level', () => {
  setFixture(real)
  const page = text(render(h(Workbench, {})))
  assert.match(page, /Workbench Models Datasets Runs/)
  assert.match(page, /Toy roster toy stock 3 · 2 hard · 1 soft 1/)
})

test('a model shows its constraints grouped, with the profile’s weights and its demos', () => {
  setFixture(real)
  const page = text(render(h(Workbench, { start: { view: 'model', id: 'toy' } })))
  assert.match(page, /New dataset from a demo: WEEK/)
  assert.match(page, /Standard \(default\)/)
  assert.match(page, /Lenient/)
  assert.doesNotMatch(page, /Make default/)
  assert.match(page, /Coverage · 1/)
  assert.match(page, /Unassigned shift .*hard/)
  assert.match(page, /Standard is read-only/)
})

test('a solved run shows its score, the gain, feasibility, KPIs and every constraint', () => {
  setFixture(real)
  const page = text(render(h(RunView, { id: real.run.run.id })))
  assert.match(page, /completed 0 hard \/ 0 soft from -6hard\/0soft feasible/)
  assert.match(page, /Assigned shifts 6 shifts/)
  assert.match(page, /Unassigned shift hard 0/)
  assert.match(page, /Score over 2\.0 s · bands top to bottom: hard/)
})

test('the chat card of an evaluated plan lists only what costs score', () => {
  setFixture({ ...real, run: real.evaluated })
  const card = items.find(i => i.id === 'run-card')
  const page = text(render(h(() => card.data.render({ attrs: { run: real.evaluated.run.id } }))))
  assert.match(page, /infeasible/)
  assert.match(page, /What costs score .*Unassigned shift hard 6 -6 hard \/ 0 soft/)
  assert.doesNotMatch(page, /Shift for the first nurse/)
  assert.match(page, /Open in the Workbench/)
})

test('a card with a malformed run id renders nothing', () => {
  setFixture(real)
  const card = items.find(i => i.id === 'run-card')
  assert.equal(render(h(() => card.data.render({ attrs: { run: '../status' } }))), '')
})

test('compare lines up runs by score, KPI and constraint', () => {
  setFixture(real)
  const page = text(render(h(Workbench, { start: { view: 'compare', id: 'a,b' } })))
  assert.match(page, /Score -6 hard \/ 0 soft 0 hard \/ 0 soft/)
  assert.match(page, /Assigned shifts 0 shifts 6 shifts/)
})

test('a dataset offers solve and evaluate and lists its runs', () => {
  setFixture(real)
  const page = text(render(h(Workbench, { start: { view: 'dataset', id: real.dataset.dataset.id } })))
  assert.match(page, /Demo Week toy · 3 nurses · 6 shifts · demo/)
  assert.match(page, /Solve Evaluate as is/)
  assert.match(page, /solve · .* completed 0 hard \/ 0 soft/)
})

test('a missing engine says how to install it', () => {
  setFixture({ ...real, status: { ...real.status, engine: { ...real.status.engine, installed: false } } })
  assert.match(text(render(h(Workbench, {}))), /The SolverForge engine is not installed/)
})

test('the chip appears only while solves run', () => {
  setFixture(real)
  assert.equal(render(h(StatusChip)), '')
  setFixture({ ...real, status: { ...real.status, solving: 2, queued: 1 } })
  assert.match(text(render(h(StatusChip))), /⚙ 2 solving · 1 queued/)
})

test('a constraint scored as a total shows its score with zero matches', () => {
  const run = structuredClone(real.run)
  run.run.analysis = [{ name: 'Minutes per period outside preferred range', level: 'soft', weight: 1, score: '0hard/-9870soft', match_count: 0, matches: [] }]
  setFixture({ ...real, run })
  const html = text(render(h(RunView, { id: run.run.id })))
  assert.match(html, /Minutes per period outside preferred range soft 0 0 hard \/ -9870 soft/)
})

test('a flat score history draws no chart', () => {
  assert.equal(render(h(ScoreChart, { points: [{ elapsed_ms: 0, score: '0hard/0soft' }, { elapsed_ms: 9, score: '0hard/0soft' }] })), '')
})

test('the workbench opens beside the chat on the right', () => {
  setFixture(real)
  openBeside({ view: 'runs' })
  assert.equal(opened[0].id, 'solverforge-workbench:workbench')
  assert.deepEqual(opened[0].dock, { pane: 'workspace', pos: 'right' })
})

test('the chip, the palette and chat cards open the Workbench section at the asked view', () => {
  setFixture(real)
  openPage({ view: 'run', id: real.run.run.id })
  const page = text(render(h(Workbench, {})))
  assert.match(page, /completed 0 hard \/ 0 soft/)
  assert.equal(opened.length, 0)
  assert.deepEqual(navigated, ['/workbench'])
})

test('every surface registers', () => {
  assert.deepEqual(items.map(i => i.id), ['run-card', 'page', 'nav', 'chip', 'open', 'runs', 'beside'])
})
