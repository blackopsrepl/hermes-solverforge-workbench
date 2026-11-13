/**
 * SolverForge Workbench — Desktop half.
 *
 * Surfaces: the SolverForge Workbench section (models, datasets, runs, compare) with its
 * own sidebar row and page, the same surface beside the chat on request, a ::solverforge-workbench{run="…"} card
 * in the transcript, a status-bar chip while solves run, and palette
 * commands. Every byte comes from this package's backend
 * (dashboard/plugin_api.py) through ctx.rest. The only writes are solve,
 * evaluate, cancel, new dataset from a demo, and saving a profile, each a
 * button the user clicks.
 */

import {
  Badge,
  Button,
  EmptyState,
  host,
  PALETTE_AREA,
  queryClient,
  SandboxedFrame,
  ROUTES_AREA,
  SegmentedControl,
  SIDEBAR_NAV_AREA,
  STATUSBAR_AREAS,
  TRANSCRIPT_DIRECTIVE_AREA,
  useMutation,
  useQuery
} from '@hermes/plugin-sdk'
import { useEffect, useState } from 'react'
import { jsx, jsxs } from 'react/jsx-runtime'

const ID = 'solverforge-workbench'
const ROUTE = '/solverforge-workbench'
const LIVE = ['queued', 'solving']

let rest = () => Promise.reject(new Error('SolverForge Workbench not loaded'))

// One element: a single child goes in as is, several as a static list.
// React reads `key` from jsx()'s third argument; inside props it is dropped.
const e = (type, props, ...children) => {
  const kids = children.filter(c => c !== null && c !== undefined && c !== false)
  const { key, ...rest } = props || {}
  return (kids.length > 1 ? jsxs : jsx)(type, { ...rest, children: kids.length > 1 ? kids : kids[0] }, key)
}
const tone = (color, size = '0.75rem') => ({ color: `var(${color})`, fontSize: size })
const muted = tone('--ui-text-secondary')
const faint = tone('--ui-text-tertiary', '0.7rem')
const qk = (...parts) => [ID, ...parts]
const param = value => encodeURIComponent(value)
const errorText = (result, fallback) => (result && result.error) || fallback

// ---- scores ------------------------------------------------------------------

/** "-6hard/0medium/-12soft" -> [{level:'hard', value:-6}, …]. */
export const parseScore = score =>
  String(score || '')
    .split('/')
    .map(part => /^(-?[0-9.]+)([a-z]+)$/i.exec(part.trim()))
    .filter(Boolean)
    .map(m => ({ level: m[2].toLowerCase(), value: Number(m[1]) }))

const LEVEL_COLOR = { hard: '--ui-danger, #e5534b', medium: '--ui-warning, #d29922', soft: '--ui-accent' }
const levelColor = level => `var(${LEVEL_COLOR[level] || '--ui-text-secondary'})`

function ScoreText({ score }) {
  const parts = parseScore(score)
  if (!parts.length) return e('span', { style: faint }, '—')
  return e(
    'span',
    { style: { fontVariantNumeric: 'tabular-nums', fontWeight: 600 } },
    parts.map((p, i) =>
      e('span', { key: p.level, style: { color: p.value < 0 ? levelColor(p.level) : 'var(--ui-text-primary)' } },
        `${i ? ' / ' : ''}${p.value} ${p.level}`)
    )
  )
}

const STATUS_VARIANT = { completed: 'success', incomplete: 'warn', failed: 'destructive', invalid: 'destructive', solving: 'default', queued: 'muted' }
const StatusBadge = ({ status }) => e(Badge, { variant: STATUS_VARIANT[status] || 'outline' }, status)

/** Score over time: one band per level that changes, each on its own scale, hard on top. */
export function ScoreChart({ points, width = 520, height = 120 }) {
  if (!points || points.length < 2) return null
  const series = {}
  const end = Math.max(1, points[points.length - 1].elapsed_ms)
  for (const point of points) {
    for (const part of parseScore(point.score)) (series[part.level] ||= []).push([point.elapsed_ms, part.value])
  }
  const lines = Object.entries(series).filter(([, values]) => values.some(([, v]) => v !== values[0][1]))
  if (!lines.length) return null
  const pad = 6
  const band = height / lines.length
  return e(
    'figure',
    { style: { margin: 0 } },
    e(
      'svg',
      { role: 'img', 'aria-label': 'Score over time', viewBox: `0 0 ${width} ${height}`, width: '100%', height,
        preserveAspectRatio: 'none', style: { display: 'block' } },
      lines.map(([level, values], n) => {
        const low = Math.min(...values.map(([, v]) => v))
        const high = Math.max(...values.map(([, v]) => v))
        const span = high - low || 1
        const top = n * band
        const d = values
          .map(([t, v], i) => `${i ? 'L' : 'M'}${(pad + (t / end) * (width - 2 * pad)).toFixed(1)},${(top + pad + ((high - v) / span) * (band - 2 * pad)).toFixed(1)}`)
          .join('')
        return e('g', { key: level },
          n ? e('line', { x1: 0, x2: width, y1: top, y2: top, stroke: 'var(--ui-stroke-tertiary)', strokeWidth: 1, vectorEffect: 'non-scaling-stroke' }) : null,
          e('path', { d, fill: 'none', stroke: levelColor(level), strokeWidth: 1.5, vectorEffect: 'non-scaling-stroke' }))
      })
    ),
    e('figcaption', { style: faint },
      `Score over ${(end / 1000).toFixed(1)} s · bands top to bottom: ${lines.map(([level]) => level).join(', ')}`)
  )
}

// ---- data ----------------------------------------------------------------------

const useRead = (kind, query = {}, options = {}) => {
  const search = Object.entries(query).filter(([, v]) => v !== undefined && v !== null && v !== '')
  const path = `/${kind}${search.length ? '?' + search.map(([k, v]) => `${k}=${param(v)}`).join('&') : ''}`
  return useQuery({ queryKey: qk(kind, ...search.flat()), queryFn: () => rest(path), staleTime: 3_000, ...options })
}

function useAction(action, onDone) {
  return useMutation({
    mutationFn: body => rest(`/actions/${action}`, { method: 'POST', body, timeoutMs: 120_000 }),
    onSuccess: result => {
      queryClient.invalidateQueries({ queryKey: [ID] })
      if (result && result.ok === false) host.notify({ kind: 'error', message: `SolverForge Workbench: ${errorText(result, action + ' failed')}` })
      else if (onDone) onDone(result)
    },
    onError: error => host.notify({ kind: 'error', message: `SolverForge Workbench: ${error.message || error}` })
  })
}

// ---- building blocks -----------------------------------------------------------

const table = { width: '100%', borderCollapse: 'collapse', fontSize: '0.75rem' }
const cell = { padding: '6px 8px', borderBottom: '1px solid var(--ui-stroke-tertiary)', textAlign: 'left', verticalAlign: 'top' }
const head = { ...cell, ...faint, fontWeight: 600 }
const num = { ...cell, textAlign: 'right', fontVariantNumeric: 'tabular-nums' }
const rowLink = { cursor: 'pointer' }
const section = { display: 'flex', flexDirection: 'column', gap: 8 }
const bar = { display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: 8 }
const input = {
  background: 'var(--ui-bg-tertiary)', color: 'var(--ui-text-primary)', border: '1px solid var(--ui-stroke-secondary)',
  borderRadius: 4, padding: '2px 6px', fontSize: '0.75rem'
}
const Title = ({ children }) => e('h2', { style: { margin: 0, fontSize: '0.95rem', fontWeight: 600 } }, children)
const Sub = ({ children }) => e('h3', { style: { margin: '6px 0 0', fontSize: '0.8rem', fontWeight: 600 } }, children)

function Table({ columns, rows, onRow }) {
  // Fixed layout with set widths keeps a column at one place across stacked tables.
  const fixed = columns.some(c => c.width)
  return e(
    'table',
    { style: fixed ? { ...table, tableLayout: 'fixed' } : table },
    e('thead', null, e('tr', null, columns.map(c => e('th', { key: c.id, style: { ...(c.num ? { ...head, textAlign: 'right' } : head), ...(c.width ? { width: c.width } : {}) } }, c.label)))),
    e('tbody', null, rows.map(row =>
      e('tr', { key: row.key, style: onRow ? rowLink : undefined, onClick: onRow ? () => onRow(row) : undefined },
        columns.map(c => e('td', { key: c.id, style: c.num ? num : cell }, row[c.id])))
    ))
  )
}

function Loading({ query, children }) {
  if (query.isLoading) return e('div', { style: muted }, 'Loading…')
  if (!query.data) return e('div', { style: muted }, 'No answer from the SolverForge Workbench.')
  if (query.data.ok === false) return e('div', { role: 'alert', style: muted }, errorText(query.data, 'The SolverForge Workbench refused.'))
  return children(query.data)
}

const levels = counts => ['hard', 'medium', 'soft'].filter(l => counts[l]).map(l => `${counts[l]} ${l}`).join(' · ')
const size = sizes => Object.entries(sizes || {}).map(([k, v]) => `${v} ${k}`).join(' · ')
const when = stamp => (stamp ? String(stamp).replace('T', ' ').slice(0, 16) : '')

// ---- views ---------------------------------------------------------------------

function ModelsView({ go }) {
  const query = useRead('models')
  return e(Loading, { query }, data =>
    data.models.length
      ? e(Table, {
          columns: [{ id: 'name', label: 'Model' }, { id: 'kind', label: 'Kind' }, { id: 'constraints', label: 'Constraints' }, { id: 'datasets', label: 'Datasets', num: true }],
          rows: data.models.map(m => ({
            key: m.id, id: m.id,
            name: e('div', null, e('strong', null, m.name), e('div', { style: faint }, m.id)),
            kind: m.kind,
            constraints: `${m.constraints} · ${levels(m.levels)}`,
            datasets: m.datasets
          })),
          onRow: row => go({ view: 'model', id: row.id })
        })
      : e(EmptyState, { title: 'No models yet', description: 'Install the engine: type /solverforge-workbench install in the chat.' })
  )
}

function WeightCell({ value, editable, onChange }) {
  if (!editable) return e('span', null, value)
  return e('input', {
    type: 'number', min: 0, max: 1000000, step: 1, value, 'aria-label': 'Weight', style: { ...input, width: 72, textAlign: 'right' },
    onChange: event => onChange(Math.max(0, Math.min(1000000, Math.trunc(Number(event.target.value) || 0))))
  })
}

function ModelView({ id, go }) {
  const query = useRead('model', { model: id })
  const [picked, setPicked] = useState(null)
  const [draft, setDraft] = useState({})
  const [name, setName] = useState('')
  const create = useAction('dataset_create', r => r.dataset && go({ view: 'dataset', id: r.dataset.id }))
  const save = useAction('profile_save', r => { setDraft({}); setName(''); r.profile && setPicked(r.profile.id) })
  const makeDefault = useAction('profile_default')
  return e(Loading, { query }, ({ model }) => {
    const profile = model.profiles.find(p => p.id === picked) || model.profiles.find(p => p.id === model.default_profile) || model.profiles[0]
    const weights = { ...(profile ? profile.weights : {}), ...draft }
    const editable = model.weights
    const changed = Object.keys(draft).length > 0
    const groups = []
    for (const c of model.constraints) {
      const group = c.group || 'Constraints'
      let found = groups.find(g => g.name === group)
      if (!found) groups.push((found = { name: group, items: [] }))
      found.items.push(c)
    }
    const settings = { weights, params: profile ? profile.params : {}, solve: profile ? profile.solve : {} }
    return e(
      'div', { style: section },
      e(Title, null, model.name),
      e('div', { style: muted }, `${model.id} · ${model.kind} · version ${model.version} · ${model.constraints.length} constraints`),
      model.demos.length
        ? e('div', { style: bar }, e('span', { style: faint }, 'New dataset from a demo:'),
            model.demos.map(d => e(Button, { key: d, size: 'xs', variant: 'outline', disabled: create.isPending,
              onClick: () => create.mutate({ model: model.id, demo: d }) }, d)))
        : null,
      e(Sub, null, 'Profile'),
      e('div', { style: bar },
        e('select', { value: profile ? profile.id : '', style: input, 'aria-label': 'Profile',
          onChange: event => { setPicked(event.target.value); setDraft({}) } },
          model.profiles.map(p => e('option', { key: p.id, value: p.id }, `${p.name}${p.id === model.default_profile ? ' (default)' : ''}`))),
        profile && profile.id !== model.default_profile
          ? e(Button, { size: 'xs', variant: 'text', onClick: () => makeDefault.mutate({ model: model.id, profile: profile.id }) }, 'Make default')
          : null,
        profile && !profile.read_only && changed
          ? e(Button, { size: 'xs', disabled: save.isPending, onClick: () => save.mutate({ model: model.id, name: profile.name, description: profile.description,
              profile: profile.id, expected_revision: profile.revision, settings }) }, 'Save')
          : null,
        changed
          ? e('span', { style: bar },
              e('input', { value: name, placeholder: 'New profile name', style: input, 'aria-label': 'New profile name', onChange: event => setName(event.target.value) }),
              e(Button, { size: 'xs', variant: 'outline', disabled: !name.trim() || save.isPending,
                onClick: () => save.mutate({ model: model.id, name: name.trim(), settings }) }, 'Save as new'),
              e(Button, { size: 'xs', variant: 'text', onClick: () => setDraft({}) }, 'Discard'))
          : null),
      profile && profile.read_only && editable ? e('div', { style: faint }, 'Standard is read-only: change a weight, then save it as a new profile.') : null,
      !editable ? e('div', { style: faint }, 'This model has no adjustable weights.') : null,
      groups.map(g => e('div', { key: g.name, style: section },
        e(Sub, null, `${g.name} · ${g.items.length}`),
        e(Table, {
          columns: [{ id: 'name', label: 'Constraint' }, { id: 'level', label: 'Level', width: 96 }, { id: 'weight', label: 'Weight', num: true, width: 104 }],
          rows: g.items.map(c => ({
            key: c.name,
            name: e('div', null, c.name, c.description ? e('div', { style: faint }, c.description) : null),
            level: e('span', { style: { color: levelColor(c.level) } }, c.level),
            weight: e(WeightCell, { value: weights[c.name] ?? 1, editable, onChange: v => setDraft({ ...draft, [c.name]: v }) })
          }))
        })))
    )
  })
}

function DatasetsView({ go }) {
  const query = useRead('datasets')
  return e(Loading, { query }, data =>
    data.datasets.length
      ? e(Table, {
          columns: [{ id: 'name', label: 'Dataset' }, { id: 'model', label: 'Model' }, { id: 'size', label: 'Size' }, { id: 'latest', label: 'Latest run' }],
          rows: data.datasets.map(d => ({
            key: d.id, id: d.id,
            name: e('div', null, e('strong', null, d.name), e('div', { style: faint }, [d.id, ...(d.tags || [])].join(' · '))),
            model: d.model_id,
            size: size(d.size),
            latest: d.latest_run ? e('div', null, e(ScoreText, { score: d.latest_run.score }), e('div', { style: faint }, d.latest_run.status)) : e('span', { style: faint }, 'not solved')
          })),
          onRow: row => go({ view: 'dataset', id: row.id })
        })
      : e(EmptyState, { title: 'No datasets yet', description: 'Open a model and start from a demo, or ask Hermes to load your data.' })
  )
}

function RunsTable({ runs, go, selected, onSelect }) {
  return e(Table, {
    columns: [
      ...(onSelect ? [{ id: 'pick', label: '' }] : []),
      { id: 'started', label: 'Run' }, { id: 'status', label: 'Status' }, { id: 'score', label: 'Score' }
    ],
    rows: runs.map(r => ({
      key: r.id, id: r.id,
      pick: onSelect ? e('input', { type: 'checkbox', checked: selected.includes(r.id), 'aria-label': `Compare ${r.id}`,
        onClick: event => event.stopPropagation(), onChange: () => onSelect(r.id) }) : null,
      started: e('div', null, `${r.kind} · ${when(r.created_at)}`, e('div', { style: faint }, `${r.model_id} · ${r.id}`)),
      status: e(StatusBadge, { status: r.status }),
      score: e(ScoreText, { score: r.score })
    })),
    onRow: row => go({ view: 'run', id: row.id })
  })
}

function DatasetView({ id, go }) {
  const query = useRead('dataset', { dataset: id }, { refetchInterval: 2_000 })
  const [profile, setProfile] = useState('')
  const [seconds, setSeconds] = useState('')
  const [selected, setSelected] = useState([])
  const solve = useAction('solve', r => r.run && go({ view: 'run', id: r.run.id }))
  const evaluate = useAction('evaluate', r => r.run && go({ view: 'run', id: r.run.id }))
  return e(Loading, { query }, ({ dataset }) => {
    const overrides = seconds ? { solve: { seconds: Number(seconds) } } : undefined
    const body = { dataset: dataset.id, ...(profile ? { profile } : {}), ...(overrides ? { overrides } : {}) }
    return e(
      'div', { style: section },
      e(Title, null, dataset.name),
      e('div', { style: muted }, `${dataset.model_id} · ${size(dataset.size)} · ${dataset.origin}${dataset.parent_id ? ` · from ${dataset.parent_id}` : ''}`),
      e('div', { style: bar },
        e(ProfilePicker, { model: dataset.model_id, value: profile, onChange: setProfile }),
        e('input', { type: 'number', min: 1, placeholder: 'seconds', value: seconds, style: { ...input, width: 80 }, 'aria-label': 'Time limit in seconds',
          onChange: event => setSeconds(event.target.value ? String(Math.max(1, Math.trunc(Number(event.target.value) || 1))) : '') }),
        e(Button, { size: 'xs', disabled: solve.isPending, onClick: () => solve.mutate(body) }, solve.isPending ? 'Queuing…' : 'Solve'),
        e(Button, { size: 'xs', variant: 'outline', disabled: evaluate.isPending, onClick: () => evaluate.mutate(body) }, 'Evaluate as is'),
        selected.length >= 2 ? e(Button, { size: 'xs', variant: 'secondary', onClick: () => go({ view: 'compare', id: selected.join(',') }) }, `Compare ${selected.length}`) : null),
      e(DatasetSchedule, { dataset }),
      dataset.runs.length
        ? e(RunsTable, { runs: dataset.runs, go, selected,
            onSelect: run => setSelected(selected.includes(run) ? selected.filter(r => r !== run) : [...selected, run]) })
        : e('div', { style: muted }, 'No runs yet.')
    )
  })
}

function DatasetSchedule({ dataset }) {
  const [shown, setShown] = useState(false)
  if (!useHasSchedule(dataset.model_id)) return null
  return e('div', { style: section },
    e('div', null, e(Button, { size: 'xs', variant: 'text', onClick: () => setShown(!shown) }, shown ? 'Hide the input schedule' : 'Show the input schedule')),
    shown ? e(Schedule, { dataset: dataset.id }) : null)
}

function ProfilePicker({ model, value, onChange }) {
  const query = useRead('model', { model })
  const profiles = query.data && query.data.ok ? query.data.model.profiles : []
  return e('select', { value, style: input, 'aria-label': 'Profile', onChange: event => onChange(event.target.value) },
    e('option', { value: '' }, 'Default profile'),
    profiles.map(p => e('option', { key: p.id, value: p.id }, p.name)))
}

/** The plan drawn as a schedule (solverforge-ui's timeline) in a sandboxed frame. */
export function Schedule({ run, dataset, version, height = 560 }) {
  const query = useRead('view', { run, dataset, v: version })
  const data = query.data
  if (query.isLoading) return e('div', { style: muted }, 'Drawing the schedule…')
  if (!data || data.ok === false) return e('div', { style: faint }, errorText(data, 'The schedule could not be drawn.'))
  return e(SandboxedFrame, {
    src: 'data:text/html;charset=utf-8,' + encodeURIComponent(data.document),
    sandbox: 'allow-scripts',
    title: 'Schedule',
    style: { width: '100%', height, border: '1px solid var(--ui-stroke-secondary)', borderRadius: 10, display: 'block', background: '#fff' }
  })
}

function useHasSchedule(model) {
  const query = useRead('model', { model })
  return Boolean(query.data && query.data.ok && query.data.model.schedule_view)
}

function Matches({ item }) {
  if (!item.matches || !item.matches.length) {
    // Range, balance and window rules are scored as one total per employee or period, not per match.
    const scored = parseScore(item.score).some(p => p.value !== 0)
    return e('div', { style: faint }, item.match_count ? 'Matches are counted, not listed, for this constraint.' : scored ? 'Scored as a total; this rule has no individual matches.' : 'No matches.')
  }
  return e('ul', { style: { margin: '4px 0', paddingLeft: 18, ...muted } },
    item.matches.map((m, i) => e('li', { key: i },
      e(ScoreText, { score: m.score }), ' · ', m.entities.map(x => `${x.type} ${x.text}`).join(' ↔ '))),
    item.match_count > item.matches.length ? e('li', { style: faint }, `… ${item.match_count - item.matches.length} more`) : null)
}

export function RunView({ id, compact = false, go }) {
  const [open, setOpen] = useState(null)
    const query = useRead('run', { run: id, max_matches: 10 }, {
    refetchInterval: q => (q && q.state && q.state.data && q.state.data.run && LIVE.includes(q.state.data.run.status) ? 1_500 : false)
  })
  const cancel = useAction('cancel')
  return e(Loading, { query }, ({ run }) => {
    const live = LIVE.includes(run.status)
    const costing = (run.analysis || []).filter(c => parseScore(c.score).some(p => p.value !== 0))
    const rows = compact ? costing : run.analysis || []
    const drawable = !compact && run.status !== 'queued' && run.model_id
    return e(
      'div', { style: section, 'data-run': run.id },
      e('div', { style: bar },
        e(StatusBadge, { status: run.status }),
        e(ScoreText, { score: run.score }),
        run.initial_score && run.initial_score !== run.score ? e('span', { style: faint }, `from ${run.initial_score}`) : null,
        run.feasible === 1 ? e(Badge, { variant: 'success' }, 'feasible') : run.feasible === 0 ? e(Badge, { variant: 'destructive' }, 'infeasible') : null,
        live ? e(Button, { size: 'xs', variant: 'outline', disabled: cancel.isPending, onClick: () => cancel.mutate({ run: run.id }) }, 'Stop') : null,
        compact ? e(Button, { size: 'xs', variant: 'text', onClick: () => openPage({ view: 'run', id: run.id }) }, 'Open in the SolverForge Workbench') : null),
      e('div', { style: faint }, `${run.kind} of ${run.dataset_id} · ${run.model_id} · ${run.id}`),
      run.error ? e('div', { role: 'alert', style: muted }, run.error.message,
        run.error.issues ? e('ul', { style: { margin: '4px 0', paddingLeft: 18 } }, run.error.issues.map((i, n) => e('li', { key: n }, `${i.severity || ''} ${i.code}: ${i.message}`))) : null) : null,
      e(ScoreChart, { points: run.points }),
      run.kpis && run.kpis.length
        ? e('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))', gap: 8 } },
            run.kpis.map(k => e('div', { key: k.id, style: { border: '1px solid var(--ui-stroke-tertiary)', borderRadius: 6, padding: 8 } },
              e('div', { style: faint }, k.label),
              e('div', { style: { fontWeight: 600, fontVariantNumeric: 'tabular-nums' } }, `${Number(k.value).toLocaleString()} ${k.unit || ''}`))))
        : null,
      drawable ? e(RunSchedule, { run }) : null,
      rows.length
        ? e(Table, {
            columns: [{ id: 'name', label: compact ? 'What costs score' : 'Constraint' }, { id: 'level', label: 'Level' }, { id: 'count', label: 'Matches', num: true }, { id: 'score', label: 'Score', num: true }],
            rows: rows.map(c => ({
              key: c.name, name: e('div', null, c.name, open === c.name ? e(Matches, { item: c }) : null),
              level: e('span', { style: { color: levelColor(c.level) } }, c.level),
              count: c.match_count, score: e(ScoreText, { score: c.score })
            })),
            onRow: row => setOpen(open === row.key ? null : row.key)
          })
        : live ? e('div', { style: muted }, 'Solving…') : null,
      go && run.dataset_id ? e('div', null, e(Button, { size: 'xs', variant: 'text', onClick: () => go({ view: 'dataset', id: run.dataset_id }) }, 'Dataset and its runs')) : null
    )
  })
}

function RunSchedule({ run }) {
  if (!useHasSchedule(run.model_id)) return null
  // The score names the plan: a better plan redraws, an unchanged one keeps its frame.
  return e('div', { style: section }, e(Sub, null, 'Schedule'), e(Schedule, { run: run.id, version: `${run.status}:${run.score}` }))
}

function CompareView({ ids }) {
  const query = useRead('compare', { runs: ids })
  return e(Loading, { query }, data => {
    const columns = [{ id: 'name', label: '' }, ...data.runs.map((r, i) => ({ id: `r${i}`, label: `${r.profile || 'no profile'} · ${r.id}`, num: true }))]
    const row = (key, name, values) => ({ key, name, ...Object.fromEntries(values.map((v, i) => [`r${i}`, v])) })
    return e('div', { style: section },
      e(Title, null, 'Compare runs'),
      e(Table, { columns, rows: [
        row('score', e('strong', null, 'Score'), data.runs.map(r => e(ScoreText, { score: r.score }))),
        row('dataset', 'Dataset', data.runs.map(r => r.dataset)),
        row('time', 'Solve time', data.runs.map(r => (r.elapsed_ms ? `${(r.elapsed_ms / 1000).toFixed(1)} s` : '—'))),
        ...data.kpis.map(k => row(`kpi:${k.id}`, k.label, k.values.map(v => (v == null ? '—' : `${Number(v).toLocaleString()} ${k.unit}`)))),
        ...data.constraints.map(c => row(`c:${c.name}`, c.name, c.scores.map(s => e(ScoreText, { score: s }))))
      ] }))
  })
}

function RunsView({ go }) {
  const query = useRead('runs', {}, { refetchInterval: 3_000 })
  return e(Loading, { query }, data =>
    data.runs.length ? e(RunsTable, { runs: data.runs, go }) : e('div', { style: muted }, 'No runs yet.'))
}

const TABS = [
  { id: 'models', label: 'Models' },
  { id: 'datasets', label: 'Datasets' },
  { id: 'runs', label: 'Runs' }
]

// Requests to show a view in the SolverForge Workbench section, from the chip, the palette or a chat card.
let requested = null
const listeners = new Set()
export function openPage(start = { view: 'models' }) {
  requested = start
  for (const listener of listeners) listener(start)
  host.navigate(ROUTE)
}

export function Workbench({ start }) {
  const [stack, setStack] = useState(() => [start || requested || { view: 'models' }])
  useEffect(() => {
    if (start) return undefined
    const listener = next => setStack([next])
    listeners.add(listener)
    return () => listeners.delete(listener)
  }, [start])
  const here = stack[stack.length - 1]
  const go = next => setStack([...stack, next])
  const tab = TABS.some(t => t.id === here.view) ? here.view : null
  const body = {
    models: () => e(ModelsView, { go }),
    model: () => e(ModelView, { id: here.id, go }),
    datasets: () => e(DatasetsView, { go }),
    dataset: () => e(DatasetView, { id: here.id, go }),
    runs: () => e(RunsView, { go }),
    run: () => e(RunView, { id: here.id, go }),
    compare: () => e(CompareView, { ids: here.id })
  }[here.view]
  return e(
    'div', { style: { display: 'flex', flexDirection: 'column', gap: 12, padding: 16, height: '100%', overflow: 'auto', color: 'var(--ui-text-primary)' } },
    e('div', { style: bar },
      e('span', { style: { fontSize: '0.95rem', fontWeight: 600 } }, 'SolverForge Workbench'),
      e(SegmentedControl, { value: tab || '', onChange: id => setStack([{ view: id }]), options: TABS }),
      e('span', { style: { flex: 1 } }),
      stack.length > 1 ? e(Button, { size: 'xs', variant: 'text', onClick: () => setStack(stack.slice(0, -1)) }, '← Back') : null),
    e(EngineNotice, null),
    body ? body() : null
  )
}

function EngineNotice() {
  const query = useRead('status')
  const engine = query.data && query.data.ok ? query.data.engine : null
  if (!engine || engine.installed) return null
  return e(EmptyState, { title: 'The SolverForge engine is not installed', description: 'Type /solverforge-workbench install in the chat; it downloads a checksum-verified binary.' })
}

// ---- beside the chat, chip, card ---------------------------------------------

let closePane = null
export function openBeside(start = { view: 'models' }) {
  if (typeof host.openWorkspace !== 'function') {
    host.navigate(ROUTE)
    return
  }
  // Re-opening the same id fronts the tab and renders the new start view.
  closePane = host.openWorkspace(`${ID}:workbench`, {
    title: 'SolverForge Workbench', minWidth: '28rem', dock: { pane: 'workspace', pos: 'right' },
    render: () => e(Workbench, { start, key: JSON.stringify(start) }),
    onClose: () => { closePane = null }
  })
}

export function StatusChip() {
  const query = useRead('status', {}, { refetchInterval: 4_000 })
  const data = query.data && query.data.ok ? query.data : null
  if (!data || !(data.solving || data.queued)) return null
  const label = `⚙ ${data.solving} solving${data.queued ? ` · ${data.queued} queued` : ''}`
  return e('button', {
    type: 'button', title: 'Show the SolverForge Workbench runs', onClick: () => openPage({ view: 'runs' }),
    style: { ...tone('--ui-accent', '0.6875rem'), padding: '0 6px', height: '100%', background: 'none', border: 0, cursor: 'pointer' }
  }, label)
}

const RUN_ID = /^[0-9a-f]{12}$/

function RunCard({ attrs, streaming }) {
  const id = attrs && (attrs.run || attrs.id)
  // Untrusted attributes: render nothing rather than guess.
  if (!RUN_ID.test(id || '') || streaming) return null
  return e('section', { style: { maxWidth: 720, border: '1px solid var(--ui-stroke-secondary)', borderRadius: 10, padding: 14 } },
    e(RunView, { id, compact: true }))
}

export default {
  id: ID,
  name: 'SolverForge Workbench',
  description: 'Optimization models: catalog, datasets, profiles, runs and their explanations.',
  register(ctx) {
    rest = (path, opts) => ctx.rest(path, opts)
    const refresh = () => queryClient.invalidateQueries({ queryKey: [ID] })
    ctx.onEvent(`plugin.${ID}.changed`, refresh)
    ctx.registerMany([
      { id: 'run-card', area: TRANSCRIPT_DIRECTIVE_AREA, data: { name: 'solverforge-workbench', render: props => jsx(RunCard, props) } },
      { id: 'page', area: ROUTES_AREA, title: 'SolverForge Workbench', data: { path: ROUTE }, render: () => jsx(Workbench, {}) },
      { id: 'nav', area: SIDEBAR_NAV_AREA, data: { path: ROUTE, label: 'SolverForge Workbench', codicon: 'beaker' } },
      { id: 'chip', area: STATUSBAR_AREAS.right, order: 127, render: () => jsx(StatusChip, {}) },
      { id: 'open', area: PALETTE_AREA, data: { id: `${ID}.open`, label: 'SolverForge Workbench: open', keywords: ['optimize', 'solver', 'models', 'schedule'], run: () => openPage() } },
      { id: 'runs', area: PALETTE_AREA, data: { id: `${ID}.runs`, label: 'SolverForge Workbench: runs', keywords: ['solve', 'runs', 'score'], run: () => openPage({ view: 'runs' }) } },
      { id: 'beside', area: PALETTE_AREA, data: { id: `${ID}.beside`, label: 'SolverForge Workbench: open beside the chat', keywords: ['optimize', 'solver', 'side', 'pane'], run: () => openBeside() } }
    ])
  }
}
