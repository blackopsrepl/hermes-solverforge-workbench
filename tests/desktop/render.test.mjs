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
const {
  default: plugin, Workbench, ScoreChart, StatusChip, FillPanel, TrendLine, parseScore, openBeside, openPage,
  parseAt, placeOf, hrefOf, runName, ago, duration, count, errorText, madeFrom
} = await import('./.build/plugin.mjs')

// React reports misuse (lost keys, bad props) through console.error; any of it fails the run.
const reactErrors = []
const consoleError = console.error
console.error = (...args) => { reactErrors.push(args.map(String).join(' ')); consoleError(...args) }

const real = Object.fromEntries(
  readdirSync(join(here, 'fixtures')).map(f => [f.replace('.json', ''), JSON.parse(readFileSync(join(here, 'fixtures', f), 'utf8'))])
)
// "just now" and "3 min ago" read against the time the fixtures were made, not the clock.
const NOW = Math.max(...real.runs.runs.map(r => Date.parse(r.created_at))) + 5_000
const RealDate = Date
globalThis.Date = class extends RealDate {
  constructor(...args) { super(...(args.length ? args : [NOW])) }
  static now() { return NOW }
}
const items = []
plugin.register({ rest: async () => ({}), onEvent() {}, registerMany: xs => items.push(...xs) })
const text = html => html.replace(/<[^>]+>/g, ' ').replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, '&').replace(/\s+/g, ' ').trim()
const render = node => renderToStaticMarkup(node)
const at = (place, fixture = real) => { setFixture(fixture); return render(h(Workbench, { start: place })) }
const runId = real.run.run.id
const datasetId = real.dataset.dataset.id
const ids = [real.run.run.id, real.dataset.dataset.id, real.experiment.experiment.id, ...real.model.model.profiles.map(p => p.id)]

test('scores parse into levels, including medium and decimals', () => {
  assert.deepEqual(parseScore('-6hard/0soft'), [{ level: 'hard', value: -6 }, { level: 'soft', value: 0 }])
  assert.deepEqual(parseScore('0hard/-2medium/-1.5soft').map(p => p.value), [0, -2, -1.5])
  assert.deepEqual(parseScore(null), [])
})

test('places round-trip through the page address', () => {
  assert.deepEqual(parseAt('/m/field-service-routing/runs/abc'), { page: 'model', model: 'field-service-routing', tab: 'runs', id: 'abc', extra: undefined })
  assert.deepEqual(parseAt(placeOf('toy')), { page: 'model', model: 'toy', tab: 'overview', id: undefined, extra: undefined })
  assert.deepEqual(parseAt('/m/toy/runs/compare/a,b').extra, 'a,b')
  assert.equal(parseAt('/runs').page, 'runs')
  assert.deepEqual(parseAt('/run/abc'), { page: 'find', kind: 'run', id: 'abc' })
  assert.equal(parseAt('').page, 'home')
  assert.equal(parseAt('/nonsense').page, 'home')
  assert.equal(hrefOf('/m/toy/runs'), '/solverforge-workbench?at=%2Fm%2Ftoy%2Fruns')
})

test('names, times and counts read as words', () => {
  const now = new Date('2026-11-18T20:00:00')
  assert.equal(runName({ created_at: '2026-11-18T19:56:00', kind: 'solve' }, now), 'Run 19:56')
  assert.equal(runName({ created_at: '2026-11-03T14:05:00', kind: 'evaluate' }, now), 'Check 3 Nov 14:05')
  assert.equal(runName({ label: 'Before the holiday' }, now), 'Before the holiday')
  assert.equal(ago('2026-11-18T19:48:00', now), '12 min ago')
  assert.equal(ago('2026-11-17T14:05:00', now), 'yesterday 14:05')
  assert.equal(duration(298_000), '4:58')
  assert.equal(duration(3_720_000), '1 h 02 min')
  assert.equal(count(1, 'run'), '1 run')
  assert.equal(count(1200, 'dataset'), '1,200 datasets')
  assert.equal(count(2, 'strategy'), '2 strategies')
  assert.equal(madeFrom({ origin: 'run', parent_name: 'Turin' }), 'Replan of Turin')
  assert.equal(errorText({ ok: false, code: 'busy', error: 'x' }), 'Something is still solving here. Stop it first.')
  assert.equal(errorText({ ok: false, error: 'unknown field weights' }), 'Unknown field weights.')
})

test('home shows each model as a card with its last result, and what runs now', () => {
  const page = text(at('/home'))
  assert.match(page, /^SolverForge Workbench Models What do you want to plan\?/)
  assert.match(page, /Toy roster Stock 3 rules · 2 hard · 2 datasets Feasible Demo Week B · just now Open Datasets/)
  assert.match(page, /Running now Nothing is solving\./)
  assert.match(page, /Recent .*Demo Week Check \d\d:\d\d · Standard/)
  assert.match(page, /All runs → All experiments →/)
})

test('a solve in progress shows its time used against its limit', () => {
  const runs = structuredClone(real.runs)
  Object.assign(runs.runs[0], { status: 'solving', seconds: 60, started_at: new Date(Date.now() - 30_000).toISOString() })
  const html = at('/home', { ...real, runs })
  assert.match(text(html), /Running now Solving Demo Week B 30 s of 1:00 Run \d\d:\d\d · Blind 0 hard 0 soft Recent/)
  assert.match(html, /role="progressbar"/)
})

test('a model opens on its overview: demos by name, datasets, latest result and how it plans', () => {
  const page = text(at(placeOf('toy')))
  assert.match(page, /SolverForge Workbench › Toy roster Toy roster Stock model · 3 rules/)
  assert.match(page, /Overview Datasets 2 Runs 6 Profiles 3 Experiments 1 Trends/)
  assert.match(page, /Start Week A sample dataset that comes with the model\. Use this demo/)
  assert.match(page, /Your datasets All datasets → Dataset Made from Size Latest Made Demo Week B Demo 3 nurses · 6 shifts/)
  assert.match(page, /Latest result Feasible 0 hard 0 soft Demo Week B · Blind/)
  assert.match(page, /How it plans 2 hard 1 soft Coverage 1 · Rules 2 Rules and weights/)
})

test('a demo with a summary says what it holds', () => {
  const model = structuredClone(real.model)
  model.model.demo_info = [{ id: 'WEEK', name: 'A week', summary: '3 nurses and 6 shifts over one week.' }]
  assert.match(text(at(placeOf('toy'), { ...real, model })), /Start A week 3 nurses and 6 shifts over one week\. Use this demo/)
})

test('the datasets tab searches, filters by origin and sorts', () => {
  const html = at(placeOf('toy', 'datasets'))
  assert.match(html, /placeholder="Search datasets: name, tag or where it came from"/)
  assert.match(text(html), /2 datasets Made from any Demo Imported Replan Fill Edited Sort newest first by name New dataset/)
  assert.match(text(html), /Demo Week demo Demo 3 nurses · 6 shifts 0 hard 0 soft/)
})

test('a dataset says what it holds and where it came from, solves, checks, keeps the past and fills', () => {
  const html = at(placeOf('toy', 'datasets', datasetId))
  const page = text(html)
  assert.match(page, /› Toy roster › Datasets › Demo Week /)
  assert.match(page, /Demo Week demo 3 nurses · 6 shifts · Demo · just now/)
  assert.match(page, /Standard \(default\) Blind Lenient Keep everything before Check as is Solve/)
  assert.match(html, /type="datetime-local"/)
  assert.match(page, /Find places for open items/)
  assert.match(page, /Runs Run Profile Status Score Time Run \d\d:\d\d just now Blind Finished 0 hard 0 soft/)
  assert.match(html, /data-c="CopyButton" data-text="[0-9a-f]{12}" aria-label="Copy the dataset&#x27;s id"/)
})

test('a replanned dataset links to the one it came from', () => {
  const dataset = structuredClone(real.dataset)
  Object.assign(dataset.dataset, { origin: 'run', parent_id: 'parent', parent_name: 'Demo Week' })
  assert.match(text(at(placeOf('toy', 'datasets', datasetId), { ...real, dataset })), /3 nurses · 6 shifts · Replan of Demo Week · just now/)
})

test('a solved run shows its verdict, score, KPIs and what costs score', () => {
  const page = text(at(placeOf('toy', 'runs', runId)))
  assert.match(page, /› Toy roster › Runs › Demo Week › Run \d\d:\d\d /)
  assert.match(page, /Run \d\d:\d\d Rename Demo Week · Standard · just now · 2 s/)
  assert.match(page, /Finished Feasible 0 hard 0 soft from -6 hard 0 soft/)
  assert.match(page, /Score over time · top to bottom: hard, each on its own scale 2 s/)
  assert.match(page, /Assigned shifts 6 shifts/)
  assert.match(page, /What costs score .*Only what costs score \(0\) Nothing costs score\./)
})

test('a rule scored as a total shows its score with zero matches', () => {
  const run = structuredClone(real.run)
  run.run.analysis = [{ name: 'Minutes per period outside preferred range', level: 'soft', weight: 1, score: '0hard/-9870soft', match_count: 0, matches: [] }]
  assert.match(text(at(placeOf('toy', 'runs', runId), { ...real, run })), /Minutes per period outside preferred range soft 0 0 hard -9,870 soft/)
})

test('a failed run says why in a sentence', () => {
  const run = structuredClone(real.run)
  Object.assign(run.run, { status: 'invalid', score: null, error: { code: 'invalid_plan', error: 'shift 3 has no start', issues: [{ message: 'Shift 3 has no start.' }] } })
  const page = text(at(placeOf('toy', 'runs', runId), { ...real, run }))
  assert.match(page, /Data problem .*Shift 3 has no start\. Shift 3 has no start\./)
})

test('a model draws the run in its own view: schedule, routes or boards', () => {
  for (const [view, title] of [['timeline', 'Schedule'], ['routes', 'Routes'], ['assignments', 'Assignments']]) {
    const model = structuredClone(real.model)
    model.model.view = view
    const html = at(placeOf('toy', 'runs', runId), { ...real, model })
    assert.match(html, new RegExp(`<h2[^>]*>${title}</h2>`))
    assert.match(html, new RegExp(`<iframe title="${title}" data-length="\\d{4,}"`))
  }
  assert.doesNotMatch(at(placeOf('toy', 'runs', runId)), /<iframe/)
})

test('compare lines runs up by score, time, KPI and the rules that differ', () => {
  const page = text(at(placeOf('toy', 'runs', 'compare', 'a,b')))
  assert.match(page, /› Runs › Compare /)
  assert.match(page, /Demo Week · Standard · Check \d\d:\d\d Demo Week · Standard · Run \d\d:\d\d/)
  assert.match(page, /Score -6 hard 0 soft 0 hard 0 soft Solve time/)
  assert.match(page, /Assigned shifts 0 shifts 6 shifts Unassigned shift -6 hard 0 soft 0 hard 0 soft/)
})

test('the runs tab filters by status and profile and offers compare', () => {
  const page = text(at(placeOf('toy', 'runs')))
  assert.match(page, /6 runs Status any Waiting Solving Finished Stopped early Failed Data problem Profile any Standard Blind Lenient Tick 2 to 4 runs to compare them/)
  assert.match(page, /Run Dataset Profile Status Score Time/)
})

test('profiles list beside the editor; Standard is read-only and groups its rules', () => {
  const page = text(at(placeOf('toy', 'profiles')))
  assert.match(page, /3 profiles Standard default · the model’s own weights Blind 1 change · edited just now Lenient 1 change/)
  assert.match(page, /Standard keeps the model’s own weights\./)
  assert.match(page, /Rules and weights 3 .*Coverage · 1 Rule Level Weight Unassigned shift Every shift needs a nurse\. hard/)
  assert.doesNotMatch(page, /Make default/)
  const blind = real.model.model.profiles.find(p => p.name === 'Blind')
  const other = text(at(placeOf('toy', 'profiles', blind.id)))
  assert.match(other, /› Profiles › Blind /)
  assert.match(other, /Blind Make default/)
})

test('settings get their own tab beside the rules', () => {
  const model = structuredClone(real.model)
  model.model.parameters = [
    { name: 'speed', kind: 'number', default: 40, label: 'Speed', unit: 'km/h', description: 'How fast vans go' },
    { name: 'holidays', kind: 'array', default: ['2027-01-01'], label: 'Holidays' },
    { name: 'strict', kind: 'boolean', default: true, label: 'Strict' }
  ]
  assert.match(text(at(placeOf('toy', 'profiles'), { ...real, model })), /Rules and weights 3 Settings 3/)
})

test('an experiment ranks its profiles and shows each dataset × profile cell', () => {
  const x = real.experiment.experiment
  const page = text(at(placeOf('toy', 'experiments', x.id)))
  assert.match(page, /› Experiments › Experiment 1 /)
  assert.match(page, /Done 2 datasets × 2 profiles × 1 · judged by Standard/)
  assert.match(page, /# Profile Feasible Average place 1 Standard 2 of 2 1 2 Blind 2 of 2 2/)
  assert.match(page, /By dataset Dataset Standard Blind Demo Week 0 hard 0 soft 1 of 1 feasible/)
  assert.match(text(at(placeOf('toy', 'experiments'))), /1 experiment New experiment Experiment Datasets × profiles × repeats Solves Started Experiment 1 2 × 2 × 1 Finished/)
})

test('a new experiment starts from the model and the dataset it was opened on', () => {
  const html = at(placeOf('toy', 'experiments', 'new', datasetId))
  const page = text(html)
  assert.match(page, /› Experiments › New experiment /)
  assert.match(page, /Datasets Demo Week B · 3 nurses · 6 shifts Demo Week · 3 nurses · 6 shifts Profiles Standard Blind Lenient/)
  assert.match(page, /Pick datasets and profiles/)
  assert.equal((html.match(/checked=""/g) || []).length, 1)
})

test('trends draw every KPI across finished solves', () => {
  const page = text(at(placeOf('toy', 'trends')))
  assert.match(page, /5 finished solves · 100% feasible/)
  assert.match(page, /Assigned shifts · 5 solves · latest 6 shifts/)
  assert.equal(render(h(TrendLine, { kpi: { label: 'X', unit: '', points: [] } })).includes('no data'), true)
})

test('all runs and all experiments span models and filter by model', () => {
  const runs = text(at('/runs'))
  assert.match(runs, /SolverForge Workbench › All runs All runs 6 runs Model any Toy roster Status any/)
  assert.match(runs, /Run Model Dataset Profile Status Score Time Run \d\d:\d\d just now Toy roster Demo Week B Blind Finished/)
  assert.match(text(at('/experiments')), /All experiments 1 experiment Model any Toy roster Pick a model to start a new experiment/)
})

test('no screen shows an id outside its copy button', () => {
  const places = ['/home', placeOf('toy'), placeOf('toy', 'datasets'), placeOf('toy', 'datasets', datasetId), placeOf('toy', 'runs'),
    placeOf('toy', 'runs', runId), placeOf('toy', 'profiles'), placeOf('toy', 'experiments'), placeOf('toy', 'experiments', real.experiment.experiment.id),
    placeOf('toy', 'trends'), '/runs', '/experiments']
  for (const place of places) {
    const page = text(at(place))
    for (const id of ids) assert.ok(!page.includes(id), `${place} shows ${id}`)
  }
})

test('every list has a search field', () => {
  for (const place of ['/home', placeOf('toy', 'datasets'), placeOf('toy', 'runs'), placeOf('toy', 'profiles'), placeOf('toy', 'experiments'), '/runs', '/experiments']) {
    assert.match(at(place), /placeholder="Search /, place)
  }
})

test('a missing engine says how to install it', () => {
  assert.match(text(at('/home', { ...real, status: { ...real.status, engine: { ...real.status.engine, installed: false } } })), /The SolverForge engine is not installed/)
})

test('a read that fails says so in a sentence and offers a retry', () => {
  const page = text(at(placeOf('toy', 'runs', runId), { ...real, run: { ok: false, code: 'unknown_run', error: 'no run abc' } }))
  assert.match(page, /This run no longer exists\. Try again/)
})

test('the chip appears only while solves run', () => {
  setFixture(real)
  assert.equal(render(h(StatusChip)), '')
  setFixture({ ...real, status: { ...real.status, solving: 2, queued: 1 } })
  assert.match(text(render(h(StatusChip))), /⚙ 2 solving · 1 waiting/)
})

test('the chat card of an evaluated plan lists only what costs score', () => {
  setFixture({ ...real, run: real.evaluated })
  const card = items.find(i => i.id === 'run-card')
  const page = text(render(h(() => card.data.render({ attrs: { run: real.evaluated.run.id } }))))
  assert.match(page, /Demo Week · Check \d\d:\d\d Finished Breaks hard rules -6 hard 0 soft/)
  assert.match(page, /What costs score Score Unassigned shift -6 hard 0 soft/)
  assert.doesNotMatch(page, /Shift for the first nurse/)
  assert.match(page, /Open in the SolverForge Workbench/)
})

test('a card with a malformed run id renders nothing', () => {
  setFixture(real)
  const card = items.find(i => i.id === 'run-card')
  assert.equal(render(h(() => card.data.render({ attrs: { run: '../status' } }))), '')
})

test('a flat score history draws no chart', () => {
  assert.equal(render(h(ScoreChart, { points: [{ elapsed_ms: 0, score: '0hard/0soft' }, { elapsed_ms: 9, score: '0hard/0soft' }] })), '')
})

test('fill ranks places for every open item, each with its score change and an apply button', () => {
  setFixture(real)
  const page = text(render(h(FillPanel, { dataset: datasetId, open: true })))
  const first = real.fill.items[0]
  assert.match(page, /Fill · 6 open/)
  assert.match(page, new RegExp(`${first.label} Place Change Rules it moves`))
  assert.match(page, new RegExp(`${first.options[0].label} · breaks a hard rule 1 hard 0 soft Unassigned shift 1hard/0soft Apply`))
  assert.equal((render(h(FillPanel, { dataset: datasetId, open: true })).match(/<button/g) || []).length, 18)
  assert.equal(text(render(h(FillPanel, { dataset: datasetId }))), 'Find places for open items')
})

test('the chip, the palette and chat cards open exact places in the section', () => {
  setFixture(real)
  navigated.length = 0
  openPage(placeOf('toy', 'runs', runId))
  assert.deepEqual(navigated, [hrefOf(`/m/toy/runs/${runId}`)])
  assert.equal(opened.length, 0)
})

test('the workbench opens beside the chat on the right, at the asked place', () => {
  setFixture(real)
  openBeside('/runs')
  assert.equal(opened[0].id, 'solverforge-workbench:workbench')
  assert.deepEqual(opened[0].dock, { pane: 'workspace', pos: 'right' })
  assert.match(text(render(opened[0].render())), /All runs/)
})

test('a custom model shows its project, and says when the project changed since its build', () => {
  const models = structuredClone(real.models)
  Object.assign(models.models[0], { kind: 'custom', source: '/home/user/crew-tasks', changed: true, summary: '' })
  const model = structuredClone(real.model)
  Object.assign(model.model, { kind: 'custom', source: '/home/user/crew-tasks', changed: true })
  assert.match(text(at('/home', { ...real, models, model })), /Toy roster Yours Project changed Your model, from \/home\/user\/crew-tasks/)
  assert.match(text(at(placeOf('toy'), { ...real, models, model })), /From \/home\/user\/crew-tasks · the project changed since this build/)
})

test('every surface registers, with the page actions rebindable', () => {
  assert.deepEqual(items.map(i => i.id), ['run-card', 'page', 'nav', 'chip', 'open', 'runs', 'experiments', 'beside',
    'key-search', 'key-new-dataset', 'key-solve', 'key-new-experiment'])
  assert.ok(items.filter(i => i.area === 'keybinds').every(i => i.data.label.startsWith('SolverForge Workbench: ') && Array.isArray(i.data.defaults)))
})

test('rendering every surface raised no React errors', () => {
  assert.deepEqual(reactErrors, [])
})
