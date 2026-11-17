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
const { default: plugin, Workbench, RunView, ScoreChart, StatusChip, FillPanel, TrendLine, parseScore, openBeside, openPage } = await import('./.build/plugin.mjs')

// React reports misuse (lost keys, bad props) through console.error; any of it fails the run.
const reactErrors = []
const consoleError = console.error
console.error = (...args) => { reactErrors.push(args.map(String).join(' ')); consoleError(...args) }

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
  assert.match(page, /SolverForge Workbench Models Datasets Runs Experiments/)
  assert.match(page, /Toy roster toy Stock 3 · 2 hard · 1 soft 2/)
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
  assert.match(page, /3 constraints/)
  assert.match(renderToStaticMarkup(h(Workbench, { start: { view: 'model', id: 'toy' } })), /placeholder="Search constraints: name, group, level or rule"/)
})

test('a solved run shows its score, the gain, feasibility, KPIs and every constraint', () => {
  setFixture(real)
  const page = text(render(h(RunView, { id: real.run.run.id })))
  assert.match(page, /completed 0 hard \/ 0 soft from -6hard\/0soft feasible/)
  assert.match(page, /Assigned shifts 6 shifts/)
  assert.match(page, /Unassigned shift hard 0/)
  assert.match(page, /Score over 2\.0 s · bands top to bottom: hard/)
  assert.match(page, /3 constraints Only what costs score \(0\)/)
})

test('the chat card of an evaluated plan lists only what costs score', () => {
  setFixture({ ...real, run: real.evaluated })
  const card = items.find(i => i.id === 'run-card')
  const page = text(render(h(() => card.data.render({ attrs: { run: real.evaluated.run.id } }))))
  assert.match(page, /infeasible/)
  assert.match(page, /What costs score .*Unassigned shift hard 6 -6 hard \/ 0 soft/)
  assert.doesNotMatch(page, /Shift for the first nurse/)
  assert.match(page, /Open in the SolverForge Workbench/)
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
  assert.deepEqual(navigated, ['/solverforge-workbench'])
})

test('a model with a timeline view draws the run as a schedule in a sandboxed frame', () => {
  const model = structuredClone(real.model)
  model.model.view = 'timeline'
  setFixture({ ...real, model })
  const html = render(h(RunView, { id: real.run.run.id }))
  assert.match(text(html), /Schedule/)
  assert.match(html, /<iframe title="Schedule" data-length="\d{6,}"/)
})

test('a custom model shows its project, and says when the project changed since its build', () => {
  const models = structuredClone(real.models)
  Object.assign(models.models[0], { kind: 'custom', source: '/home/user/crew-tasks', changed: true })
  const model = structuredClone(real.model)
  Object.assign(model.model, { kind: 'custom', source: '/home/user/crew-tasks', changed: true })
  setFixture({ ...real, models, model })
  const list = text(render(h(Workbench, { start: { view: 'models' } })))
  assert.match(list, /Toy roster \/home\/user\/crew-tasks Your model Project changed/)
  const page = text(render(h(Workbench, { start: { view: 'model', id: 'toy' } })))
  assert.match(page, /From \/home\/user\/crew-tasks · the project changed since this build: ask Hermes to register it again\./)
})

test('a model with an assignments view draws the run as boards in a sandboxed frame', () => {
  const model = structuredClone(real.model)
  model.model.view = 'assignments'
  setFixture({ ...real, model })
  const html = render(h(RunView, { id: real.run.run.id }))
  assert.match(text(html), /Assignments/)
  assert.match(html, /<iframe title="Assignments"/)
})

test('a model without one shows no schedule', () => {
  setFixture(real)
  assert.doesNotMatch(render(h(RunView, { id: real.run.run.id })), /<iframe/)
})

test('a model with a routes view draws the run as routes in a taller frame', () => {
  const model = structuredClone(real.model)
  model.model.view = 'routes'
  setFixture({ ...real, model })
  const html = render(h(RunView, { id: real.run.run.id }))
  assert.match(text(html), /Routes/)
  assert.match(html, /<iframe title="Routes"/)
})

test('a model lists its parameters with their values, trends and experiments', () => {
  const model = structuredClone(real.model)
  model.model.parameters = [{ name: 'speed_kmh', kind: 'number', default: 40, description: 'Driving speed without a road network' }]
  setFixture({ ...real, model })
  const page = text(render(h(Workbench, { start: { view: 'model', id: 'toy' } })))
  assert.match(page, /Parameters · 1 Parameter Kind Value speed_kmh Driving speed without a road network number/)
  assert.match(page, /Trends New experiment/)
  assert.match(page, /change a weight or a parameter/)
})

test('a dataset can freeze the past, fill open items and start an experiment', () => {
  setFixture(real)
  const html = render(h(Workbench, { start: { view: 'dataset', id: real.dataset.dataset.id } }))
  assert.match(html, /type="datetime-local"/)
  assert.match(text(html), /Freeze before .*Solve Evaluate as is Experiment…/)
  assert.match(text(html), /Find places for open items/)
})

test('fill ranks places for every open item, each with its score change and an apply button', () => {
  setFixture(real)
  const page = text(render(h(FillPanel, { dataset: real.dataset.dataset.id, open: true })))
  const first = real.fill.items[0]
  assert.match(page, /Fill · 6 open/)
  assert.match(page, new RegExp(`${first.label} · Shift Shift#0`))
  assert.match(page, /Place Change Constraints it moves/)
  assert.match(page, new RegExp(`${first.options[0].label} · infeasible 1 hard / 0 soft Unassigned shift 1hard/0soft Apply`))
  assert.equal((render(h(FillPanel, { dataset: real.dataset.dataset.id, open: true })).match(/<button/g) || []).length, 18)
})

test('fill starts closed until asked', () => {
  setFixture(real)
  assert.equal(text(render(h(FillPanel, { dataset: real.dataset.dataset.id }))), 'Find places for open items')
})

test('an experiment ranks its profiles and shows each dataset × profile cell', () => {
  setFixture(real)
  const x = real.experiment.experiment
  const page = text(render(h(Workbench, { start: { view: 'experiment', id: x.id } })))
  assert.match(page, /datasets × 2 profiles × 1 · judged by Standard/)
  assert.match(page, /done/)
  assert.match(page, /# Profile Feasible Average place 1 Standard 2 of 2 1 2 Blind 2 of 2/)
  assert.match(page, /Dataset Standard Blind Demo Week 0 hard \/ 0 soft median 0hard\/0soft · 1\/1 feasible/)
  const list = text(render(h(Workbench, { start: { view: 'experiments' } })))
  assert.match(list, /2 × 2 × 1 4 done/)
})

test('a new experiment counts the solves it will queue', () => {
  setFixture(real)
  const page = text(render(h(Workbench, { start: { view: 'experiment-new', id: 'toy', datasets: [real.dataset.dataset.id] } })))
  assert.match(page, /New experiment · Toy roster/)
  assert.match(page, /Pick datasets and profiles/)
  assert.match(page, /Standard Lenient|Lenient/)
})

test('trends draw every KPI across finished solves', () => {
  setFixture(real)
  const page = text(render(h(Workbench, { start: { view: 'trends', id: 'toy' } })))
  assert.match(page, /Trends · toy/)
  assert.match(page, /finished solves · 100% feasible/)
  assert.match(page, /Assigned shifts · \d+ solves · latest 6 shifts/)
  assert.equal(render(h(TrendLine, { kpi: { label: 'X', unit: '', points: [] } })).includes('no data'), true)
})

test('every surface registers', () => {
  assert.deepEqual(items.map(i => i.id), ['run-card', 'page', 'nav', 'chip', 'open', 'runs', 'beside'])
})

test('rendering every surface raised no React errors', () => {
  assert.deepEqual(reactErrors, [])
})
