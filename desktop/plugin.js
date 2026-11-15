/**
 * SolverForge Workbench — Desktop half.
 *
 * Surfaces: the SolverForge Workbench section (models, datasets, runs, experiments,
 * compare, trends) with its own sidebar row and page, the same surface beside the chat
 * on request, a ::solverforge-workbench{run="…"} card in the transcript, a status-bar
 * chip while solves run, and palette commands. Every byte comes from this package's
 * backend (dashboard/plugin_api.py) through ctx.rest. The only writes are solve,
 * evaluate, cancel, new dataset from a demo, saving a profile, applying a fill option
 * and starting or stopping an experiment, each a button the user clicks.
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

// ---- constraint search -------------------------------------------------------

const WORD_START = /[\s\-_/·:(]/

/** Fuzzy match of `needle` in `hay`. A substring always matches. Otherwise the characters
 *  appear in order, the first at a word start, and each later one either inside the word
 *  of the one before it or at the start of a later word: "rst btwn" finds "Rest between
 *  shifts" and "cdo" finds "Consecutive days off", while letters scattered across the
 *  middles of words never match. Returns { score, hits } (indices into hay) or null. */
export function fuzzyMatch(needle, hay) {
  const q = Array.from(String(needle || '').toLowerCase())
  const t = Array.from(String(hay || '').toLowerCase())
  if (!q.length) return { score: 0, hits: [] }
  const starts = t.map((_, j) => j === 0 || WORD_START.test(t[j - 1]))
  const text = t.join('')
  const exact = text.indexOf(q.join(''))
  if (exact >= 0) {
    return { score: 100 + q.length * 4 + (starts[exact] ? 40 : 0) - exact * 0.1, hits: q.map((_, i) => exact + i) }
  }
  // best[i][j]: best score with q[i] placed at t[j]; from[i][j]: where q[i-1] sat.
  const best = q.map(() => new Array(t.length).fill(-Infinity))
  const from = q.map(() => new Array(t.length).fill(-1))
  for (let j = 0; j < t.length; j++) if (t[j] === q[0] && starts[j]) best[0][j] = j === 0 ? 12 : 10
  const separator = t.map(ch => WORD_START.test(ch))
  for (let i = 1; i < q.length; i++) {
    let lead = -Infinity // best placement of q[i-1] anywhere before j
    let leadAt = -1
    let word = -Infinity // best placement of q[i-1] earlier in j's own word
    let wordAt = -1
    for (let j = 0; j < t.length; j++) {
      if (starts[j]) { word = -Infinity; wordAt = -1 }
      if (t[j] === q[i] && !separator[j]) {
        const options = [
          [j > 0 ? best[i - 1][j - 1] + 8 : -Infinity, j - 1], // the next letter
          [starts[j] ? lead + 6 : -Infinity, leadAt], // the start of a later word
          [word + 3, wordAt] // further along the same word
        ]
        for (const [score, at] of options) if (score > best[i][j]) { best[i][j] = score; from[i][j] = at }
      }
      if (best[i - 1][j] > lead) { lead = best[i - 1][j]; leadAt = j }
      if (best[i - 1][j] > word) { word = best[i - 1][j]; wordAt = j }
    }
  }
  const last = best[q.length - 1]
  let end = -1
  for (let j = 0; j < t.length; j++) if (last[j] > -Infinity && (end < 0 || last[j] > last[end])) end = j
  if (end < 0) return null
  const hits = []
  for (let i = q.length - 1, j = end; i >= 0; j = from[i][j], i--) hits.unshift(j)
  return { score: last[end], hits }
}

const FIELDS = [['name', 1], ['group', 0.6], ['level', 0.6], ['description', 0.4]]

/** Every whitespace-separated term must match some field. Results keep the input order
 *  when the query is empty, and rank best first otherwise; `hits` highlight the name. */
export function searchConstraints(items, query) {
  const terms = String(query || '').trim().split(/\s+/).filter(Boolean)
  if (!terms.length) return items.map(item => ({ item, score: 0, hits: [] }))
  const found = []
  for (const item of items) {
    let total = 0
    const hits = new Set()
    let matched = true
    let previous = -1 // last name position matched by the term before, to reward terms read in order
    for (const term of terms) {
      let best = null
      for (const [field, weight] of FIELDS) {
        const m = fuzzyMatch(term, item[field])
        if (m && (!best || m.score * weight > best.score)) best = { score: m.score * weight, field, hits: m.hits }
      }
      if (!best) { matched = false; break }
      total += best.score
      if (best.field === 'name') {
        if (best.hits[0] > previous && previous >= 0) total += 8
        previous = best.hits[best.hits.length - 1]
        best.hits.forEach(h => hits.add(h))
      }
    }
    if (matched) found.push({ item, score: total, hits: [...hits].sort((x, y) => x - y) })
  }
  return found.sort((x, y) => y.score - x.score)
}

function Highlight({ text, hits }) {
  if (!hits || !hits.length) return text
  const marks = new Set(hits)
  const parts = []
  let run = ''
  let marked = false
  const flush = () => {
    if (run) parts.push(marked ? e('mark', { key: parts.length, style: { background: 'color-mix(in srgb, var(--ui-accent) 28%, transparent)', color: 'inherit', borderRadius: 2 } }, run) : run)
    run = ''
  }
  Array.from(text).forEach((ch, i) => {
    if (marks.has(i) !== marked) { flush(); marked = marks.has(i) }
    run += ch
  })
  flush()
  return e('span', null, ...parts)
}

function SearchBox({ value, onChange, shown, total, extra }) {
  return e('div', { style: { ...bar, position: 'sticky', top: 0, zIndex: 1, background: 'var(--ui-bg-primary, var(--background))', padding: '6px 0' } },
    e('input', {
      type: 'search', value, placeholder: 'Search constraints: name, group, level or rule', 'aria-label': 'Search constraints',
      style: { ...input, flex: '1 1 280px', padding: '5px 8px', fontSize: '0.8rem' },
      onChange: event => onChange(event.target.value),
      onKeyDown: event => { if (event.key === 'Escape') onChange('') }
    }),
    e('span', { style: faint, 'aria-live': 'polite' }, value.trim() ? `${shown} of ${total}` : `${total} constraints`),
    extra || null)
}

function ParamCell({ param, value, editable, onChange }) {
  if (!editable) return e('span', null, String(value))
  if (param.kind === 'boolean') {
    return e('input', { type: 'checkbox', checked: Boolean(value), 'aria-label': param.name, onChange: event => onChange(event.target.checked) })
  }
  if (param.kind === 'string') {
    return e('input', { value: value ?? '', 'aria-label': param.name, style: { ...input, width: 140 }, onChange: event => onChange(event.target.value) })
  }
  const whole = param.kind === 'integer' || param.kind === 'duration'
  return e('input', {
    type: 'number', step: whole ? 1 : 'any', value: value ?? '', 'aria-label': param.name, style: { ...input, width: 96, textAlign: 'right' },
    onChange: event => {
      const n = Number(event.target.value)
      if (event.target.value !== '' && Number.isFinite(n)) onChange(whole ? Math.trunc(n) : n)
    }
  })
}

function ModelView({ id, go }) {
  const query = useRead('model', { model: id })
  const [picked, setPicked] = useState(null)
  const [draft, setDraft] = useState({})
  const [draftParams, setDraftParams] = useState({})
  const [name, setName] = useState('')
  const [search, setSearch] = useState('')
  const create = useAction('dataset_create', r => r.dataset && go({ view: 'dataset', id: r.dataset.id }))
  const save = useAction('profile_save', r => { setDraft({}); setDraftParams({}); setName(''); r.profile && setPicked(r.profile.id) })
  const makeDefault = useAction('profile_default')
  return e(Loading, { query }, ({ model }) => {
    const profile = model.profiles.find(p => p.id === picked) || model.profiles.find(p => p.id === model.default_profile) || model.profiles[0]
    const weights = { ...(profile ? profile.weights : {}), ...draft }
    const params = { ...(profile ? profile.params : {}), ...draftParams }
    const editable = model.weights
    const parameters = model.parameters || []
    const changed = Object.keys(draft).length > 0 || Object.keys(draftParams).length > 0
    // Groups in catalog order; while searching, groups and their rows rank by the best match.
    const results = searchConstraints(model.constraints, search)
    const groups = []
    for (const r of results) {
      const group = r.item.group || 'Constraints'
      let found = groups.find(g => g.name === group)
      if (!found) groups.push((found = { name: group, items: [], size: model.constraints.filter(c => (c.group || 'Constraints') === group).length }))
      found.items.push(r)
    }
    const settings = { weights, params, solve: profile ? profile.solve : {} }
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
          onChange: event => { setPicked(event.target.value); setDraft({}); setDraftParams({}) } },
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
              e(Button, { size: 'xs', variant: 'text', onClick: () => { setDraft({}); setDraftParams({}) } }, 'Discard'))
          : null),
      profile && profile.read_only && (editable || parameters.length) ? e('div', { style: faint }, 'Standard is read-only: change a weight or a parameter, then save it as a new profile.') : null,
      !editable ? e('div', { style: faint }, 'This model has no adjustable weights.') : null,
      parameters.length
        ? e('div', { style: section },
            e(Sub, null, `Parameters · ${parameters.length}`),
            e(Table, {
              columns: [{ id: 'name', label: 'Parameter' }, { id: 'kind', label: 'Kind', width: 96 }, { id: 'value', label: 'Value', num: true, width: 160 }],
              rows: parameters.map(prm => ({
                key: prm.name,
                name: e('div', null, prm.name, prm.description ? e('div', { style: faint }, prm.description) : null),
                kind: prm.kind === 'duration' ? 'minutes' : prm.kind,
                value: e(ParamCell, { param: prm, value: params[prm.name] ?? prm.default, editable: true,
                  onChange: v => setDraftParams({ ...draftParams, [prm.name]: v }) })
              }))
            }))
        : null,
      e('div', { style: bar },
        e(Button, { size: 'xs', variant: 'outline', onClick: () => go({ view: 'trends', id: model.id }) }, 'Trends'),
        e(Button, { size: 'xs', variant: 'outline', onClick: () => go({ view: 'experiment-new', id: model.id }) }, 'New experiment')),
      e(SearchBox, { value: search, onChange: setSearch, shown: results.length, total: model.constraints.length }),
      search.trim() && !results.length ? e('div', { style: muted }, `No constraint matches “${search.trim()}”.`) : null,
      groups.map(g => e('div', { key: g.name, style: section },
        e(Sub, null, search.trim() ? `${g.name} · ${g.items.length} of ${g.size}` : `${g.name} · ${g.size}`),
        e(Table, {
          columns: [{ id: 'name', label: 'Constraint' }, { id: 'level', label: 'Level', width: 96 }, { id: 'weight', label: 'Weight', num: true, width: 104 }],
          rows: g.items.map(({ item: c, hits }) => ({
            key: c.name,
            name: e('div', null, e(Highlight, { text: c.name, hits }), c.description ? e('div', { style: faint }, c.description) : null),
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
  const [freeze, setFreeze] = useState('')
  const [selected, setSelected] = useState([])
  const solve = useAction('solve', r => r.run && go({ view: 'run', id: r.run.id }))
  const evaluate = useAction('evaluate', r => r.run && go({ view: 'run', id: r.run.id }))
  return e(Loading, { query }, ({ dataset }) => {
    const solveSettings = { ...(seconds ? { seconds: Number(seconds) } : {}), ...(freeze ? { freeze_before: freeze.slice(0, 16) } : {}) }
    const overrides = Object.keys(solveSettings).length ? { solve: solveSettings } : undefined
    const body = { dataset: dataset.id, ...(profile ? { profile } : {}), ...(overrides ? { overrides } : {}) }
    return e(
      'div', { style: section },
      e(Title, null, dataset.name),
      e('div', { style: muted }, `${dataset.model_id} · ${size(dataset.size)} · ${dataset.origin}${dataset.parent_id ? ` · from ${dataset.parent_id}` : ''}`),
      dataset.origin === 'fill' && dataset.patch ? e('div', { style: faint }, dataset.patch.map(p => `placed ${p.item} at ${p.value}`).join(' · ')) : null,
      e('div', { style: bar },
        e(ProfilePicker, { model: dataset.model_id, value: profile, onChange: setProfile }),
        e('input', { type: 'number', min: 1, placeholder: 'seconds', value: seconds, style: { ...input, width: 80 }, 'aria-label': 'Time limit in seconds',
          onChange: event => setSeconds(event.target.value ? String(Math.max(1, Math.trunc(Number(event.target.value) || 1))) : '') }),
        e('label', { style: { ...faint, display: 'inline-flex', alignItems: 'center', gap: 4 } }, 'Freeze before',
          e('input', { type: 'datetime-local', value: freeze, style: input, 'aria-label': 'Freeze everything that starts before',
            onChange: event => setFreeze(event.target.value) })),
        e(Button, { size: 'xs', disabled: solve.isPending, onClick: () => solve.mutate(body) }, solve.isPending ? 'Queuing…' : 'Solve'),
        e(Button, { size: 'xs', variant: 'outline', disabled: evaluate.isPending, onClick: () => evaluate.mutate(body) }, 'Evaluate as is'),
        e(Button, { size: 'xs', variant: 'outline', onClick: () => go({ view: 'experiment-new', id: dataset.model_id, datasets: [dataset.id] }) }, 'Experiment…'),
        selected.length >= 2 ? e(Button, { size: 'xs', variant: 'secondary', onClick: () => go({ view: 'compare', id: selected.join(',') }) }, `Compare ${selected.length}`) : null),
      freeze ? e('div', { style: faint }, `Everything that starts before ${freeze.replace('T', ' ')} keeps its current assignment.`) : null,
      e(DatasetPlan, { dataset }),
      e(FillPanel, { dataset: dataset.id, profile, go }),
      dataset.runs.length
        ? e(RunsTable, { runs: dataset.runs, go, selected,
            onSelect: run => setSelected(selected.includes(run) ? selected.filter(r => r !== run) : [...selected, run]) })
        : e('div', { style: muted }, 'No runs yet.')
    )
  })
}

function DatasetPlan({ dataset }) {
  const [shown, setShown] = useState(false)
  const kind = useViewKind(dataset.model_id)
  if (!kind) return null
  return e('div', { style: section },
    e('div', null, e(Button, { size: 'xs', variant: 'text', onClick: () => setShown(!shown) },
      `${shown ? 'Hide' : 'Show'} the input ${kind === 'routes' ? 'routes' : 'schedule'}`)),
    shown ? e(PlanView, { dataset: dataset.id, kind }) : null)
}

/** Fill: the best places for a plan's open items, ranked by the score each reaches; one click applies one. */
export function FillPanel({ run, dataset, profile, go, open = false }) {
  const [asked, setAsked] = useState(open)
  const query = useRead('fill', { run, dataset, profile, top: 3 }, { enabled: asked })
  const apply = useAction('fill_apply', r => r.dataset && go && go({ view: 'dataset', id: r.dataset.id }))
  if (!asked) {
    return e('div', null, e(Button, { size: 'xs', variant: 'outline', onClick: () => setAsked(true) }, 'Find places for open items'))
  }
  return e(Loading, { query }, data => {
    if (!data.items.length) return e('div', { style: muted }, 'Nothing is open in this plan.')
    return e('div', { style: section },
      e(Sub, null, `Fill · ${data.items.length} open`),
      e('div', { style: faint }, 'Each option is scored by the model with the item placed; applying one makes a new dataset.'),
      data.items.map(item => e('div', { key: `${item.type}:${item.id}`, style: section },
        e('div', null, e('strong', null, item.label), e('span', { style: faint }, ` · ${item.type} ${item.id}`)),
        item.options.length
          ? e(Table, {
              columns: [{ id: 'place', label: 'Place' }, { id: 'delta', label: 'Change', num: true, width: 180 }, { id: 'why', label: 'Constraints it moves' }, { id: 'act', label: '', width: 72 }],
              rows: item.options.map(option => ({
                key: option.id || option.label,
                place: e('div', null, option.label, option.feasible ? null : e('span', { style: faint }, ' · infeasible')),
                delta: e(ScoreText, { score: option.delta }),
                why: e('span', { style: faint }, option.changes.map(c => `${c.constraint} ${c.delta}`).join(' · ') || 'no constraint changes'),
                act: e(Button, { size: 'xs', disabled: apply.isPending,
                  onClick: () => apply.mutate({ ...(run ? { run } : { dataset }), item: item.id, value: option.id, ...(profile ? { profile } : {}) }) }, 'Apply')
              }))
            })
          : e('div', { style: faint }, 'No place fits this item.'))))
  })
}

function ProfilePicker({ model, value, onChange }) {
  const query = useRead('model', { model })
  const profiles = query.data && query.data.ok ? query.data.model.profiles : []
  return e('select', { value, style: input, 'aria-label': 'Profile', onChange: event => onChange(event.target.value) },
    e('option', { value: '' }, 'Default profile'),
    profiles.map(p => e('option', { key: p.id, value: p.id }, p.name)))
}

/** The plan as its model draws it (solverforge-ui's timeline, or routes on a map over a
 *  per-vehicle timeline) in a sandboxed frame. */
export function PlanView({ run, dataset, version, kind = 'timeline' }) {
  const query = useRead('view', { run, dataset, v: version })
  const data = query.data
  const what = kind === 'routes' ? 'routes' : 'schedule'
  if (query.isLoading) return e('div', { style: muted }, `Drawing the ${what}…`)
  if (!data || data.ok === false) return e('div', { style: faint }, errorText(data, `The ${what} could not be drawn.`))
  return e(SandboxedFrame, {
    src: 'data:text/html;charset=utf-8,' + encodeURIComponent(data.document),
    sandbox: 'allow-scripts',
    title: kind === 'routes' ? 'Routes' : 'Schedule',
    style: { width: '100%', height: kind === 'routes' ? 1040 : 560, border: '1px solid var(--ui-stroke-secondary)', borderRadius: 10, display: 'block', background: '#fff' }
  })
}

/** "timeline", "routes", or null when the model draws no plan view. */
function useViewKind(model) {
  const query = useRead('model', { model })
  return (query.data && query.data.ok && query.data.model.view) || null
}

function Matches({ item }) {
  if (!item.matches || !item.matches.length) {
    // Range, balance and window rules are scored as one total per employee or period, not per match.
    const scored = parseScore(item.score).some(p => p.value !== 0)
    return e('div', { style: faint }, item.match_count ? 'Matches are counted, not listed, for this constraint.' : scored ? 'Scored as a total; this rule has no individual matches.' : 'No matches.')
  }
  return e('ul', { style: { margin: '4px 0', paddingLeft: 18, overflowWrap: 'anywhere', ...muted } },
    item.matches.map((m, i) => e('li', { key: i },
      e(ScoreText, { score: m.score }), ' · ', m.entities.map(x => x.label).join(' · '))),
    item.match_count > item.matches.length ? e('li', { style: faint }, `… ${item.match_count - item.matches.length} more`) : null)
}

export function RunView({ id, compact = false, go }) {
  const [open, setOpen] = useState(null)
  const [search, setSearch] = useState('')
  const [costlyOnly, setCostlyOnly] = useState(false)
  const query = useRead('run', { run: id, max_matches: 10 }, {
    refetchInterval: q => (q && q.state && q.state.data && q.state.data.run && LIVE.includes(q.state.data.run.status) ? 1_500 : false)
  })
  const cancel = useAction('cancel')
  const model = useRead('model', { model: query.data && query.data.ok ? query.data.run.model_id : '' },
    { enabled: Boolean(query.data && query.data.ok) })
  return e(Loading, { query }, ({ run }) => {
    const live = LIVE.includes(run.status)
    const costing = (run.analysis || []).filter(c => parseScore(c.score).some(p => p.value !== 0))
    const all = compact ? costing : run.analysis || []
    // The run's analysis has names and levels; the catalog adds group and description to search on.
    const catalog = new Map(((model.data && model.data.ok && model.data.model.constraints) || []).map(c => [c.name, c]))
    const pool = (costlyOnly ? costing : all).map(c => ({ ...catalog.get(c.name), ...c }))
    const rows = compact ? pool.map(item => ({ item, hits: [] })) : searchConstraints(pool, search)
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
      drawable ? e(RunPlan, { run }) : null,
      !compact && ['completed', 'incomplete'].includes(run.status) ? e(FillPanel, { run: run.id, go }) : null,
      !compact && all.length
        ? e(SearchBox, {
            value: search, onChange: setSearch, shown: rows.length, total: all.length,
            extra: e('label', { style: { ...faint, display: 'inline-flex', alignItems: 'center', gap: 4 } },
              e('input', { type: 'checkbox', checked: costlyOnly, onChange: event => setCostlyOnly(event.target.checked) }),
              `Only what costs score (${costing.length})`)
          })
        : null,
      !compact && all.length && !rows.length ? e('div', { style: muted }, 'No constraint matches.') : null,
      rows.length
        ? e(Table, {
            // Fixed widths: an opened constraint's matches wrap inside its name instead of squeezing the score.
            columns: [{ id: 'name', label: compact ? 'What costs score' : 'Constraint' }, { id: 'level', label: 'Level', width: 80 },
              { id: 'count', label: 'Matches', num: true, width: 80 }, { id: 'score', label: 'Score', num: true, width: compact ? 180 : 240 }],
            rows: rows.map(({ item: c, hits }) => ({
              key: c.name, name: e('div', null, e(Highlight, { text: c.name, hits }), open === c.name ? e(Matches, { item: c }) : null),
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

function RunPlan({ run }) {
  const kind = useViewKind(run.model_id)
  if (!kind) return null
  // The score names the plan: a better plan redraws, an unchanged one keeps its frame.
  return e('div', { style: section }, e(Sub, null, kind === 'routes' ? 'Routes' : 'Schedule'),
    e(PlanView, { run: run.id, kind, version: `${run.status}:${run.score}` }))
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

// ---- experiments and trends ------------------------------------------------------

function ExperimentsView({ go }) {
  const query = useRead('experiments', {}, { refetchInterval: 3_000 })
  return e(Loading, { query }, data =>
    data.experiments.length
      ? e(Table, {
          columns: [{ id: 'name', label: 'Experiment' }, { id: 'model', label: 'Model' }, { id: 'shape', label: 'Datasets × profiles × repeats' }, { id: 'progress', label: 'Solves', num: true }],
          rows: data.experiments.map(x => ({
            key: x.id, id: x.id,
            name: e('div', null, e('strong', null, x.name), e('div', { style: faint }, `${x.id} · ${when(x.created_at)}`)),
            model: x.model_id,
            shape: `${x.datasets.length} × ${x.profiles.length} × ${x.repeats}`,
            progress: x.done ? `${x.runs} done` : `${x.finished} of ${x.runs}`
          })),
          onRow: row => go({ view: 'experiment', id: row.id })
        })
      : e(EmptyState, { title: 'No experiments yet', description: 'Open a model or a dataset and start one: every dataset solved under every profile, ranked.' }))
}

function Picks({ label, options, chosen, onChange }) {
  return e('fieldset', { style: { ...section, border: '1px solid var(--ui-stroke-tertiary)', borderRadius: 6, padding: 8, margin: 0 } },
    e('legend', { style: faint }, label),
    options.length
      ? options.map(o => e('label', { key: o.id, style: { ...muted, display: 'flex', alignItems: 'center', gap: 6 } },
          e('input', { type: 'checkbox', checked: chosen.includes(o.id), onChange: () => onChange(chosen.includes(o.id) ? chosen.filter(c => c !== o.id) : [...chosen, o.id]) }),
          o.label))
      : e('div', { style: faint }, 'None yet.'))
}

function ExperimentNew({ model, datasets: initial, go }) {
  const modelQuery = useRead('model', { model })
  const datasetQuery = useRead('datasets', { model })
  const [datasets, setDatasets] = useState(initial || [])
  const [profiles, setProfiles] = useState([])
  const [repeats, setRepeats] = useState('2')
  const [seconds, setSeconds] = useState('30')
  const [judge, setJudge] = useState('')
  const [name, setName] = useState('')
  const create = useAction('experiment_create', r => r.experiment && go({ view: 'experiment', id: r.experiment.id }))
  return e(Loading, { query: modelQuery }, ({ model: m }) => {
    const all = datasetQuery.data && datasetQuery.data.ok ? datasetQuery.data.datasets : []
    const total = datasets.length * profiles.length * Number(repeats || 0)
    return e('div', { style: section },
      e(Title, null, `New experiment · ${m.name}`),
      e('div', { style: muted }, 'Every dataset is solved under every profile, repeats times each. Profiles weigh constraints differently, so each plan is scored again under the judge profile to rank them.'),
      e('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: 8 } },
        e(Picks, { label: 'Datasets', chosen: datasets, onChange: setDatasets, options: all.map(d => ({ id: d.id, label: `${d.name} · ${size(d.size)}` })) }),
        e(Picks, { label: 'Profiles', chosen: profiles, onChange: setProfiles, options: m.profiles.map(p => ({ id: p.id, label: p.name })) })),
      e('div', { style: bar },
        e('input', { value: name, placeholder: 'Name', style: input, 'aria-label': 'Experiment name', onChange: event => setName(event.target.value) }),
        e('label', { style: faint }, 'Repeats ', e('input', { type: 'number', min: 1, max: 20, value: repeats, style: { ...input, width: 56 }, 'aria-label': 'Repeats', onChange: event => setRepeats(event.target.value) })),
        e('label', { style: faint }, 'Seconds each ', e('input', { type: 'number', min: 1, value: seconds, style: { ...input, width: 64 }, 'aria-label': 'Seconds per solve', onChange: event => setSeconds(event.target.value) })),
        e('label', { style: faint }, 'Judge ',
          e('select', { value: judge, style: input, 'aria-label': 'Judge profile', onChange: event => setJudge(event.target.value) },
            e('option', { value: '' }, 'Standard'),
            m.profiles.filter(p => !p.read_only).map(p => e('option', { key: p.id, value: p.id }, p.name)))),
        e(Button, { size: 'xs', disabled: !total || total > 200 || create.isPending,
          onClick: () => create.mutate({ datasets, profiles, repeats: Math.max(1, Math.trunc(Number(repeats) || 1)), ...(name.trim() ? { name: name.trim() } : {}),
            ...(judge ? { judge } : {}), ...(Number(seconds) > 0 ? { overrides: { solve: { seconds: Math.trunc(Number(seconds)) } } } : {}) }) },
          total ? `Queue ${total} solve${total === 1 ? '' : 's'}` : 'Pick datasets and profiles')),
      total > 200 ? e('div', { role: 'alert', style: muted }, 'At most 200 solves per experiment.') : null)
  })
}

function ExperimentView({ id, go }) {
  const query = useRead('experiment', { experiment: id }, {
    refetchInterval: q => (q && q.state && q.state.data && q.state.data.experiment && !q.state.data.experiment.done ? 2_000 : false)
  })
  const cancel = useAction('experiment_cancel')
  return e(Loading, { query }, ({ experiment: x }) => {
    const finished = Object.entries(x.statuses).filter(([k]) => !LIVE.includes(k)).reduce((n, [, v]) => n + v, 0)
    const runs = Object.values(x.statuses).reduce((n, v) => n + v, 0)
    const cell = (dataset, profile) => x.cells.find(c => c.dataset_id === dataset && c.profile_id === profile)
    return e('div', { style: section },
      e(Title, null, x.name),
      e('div', { style: muted }, `${x.model_id} · ${x.datasets.length} datasets × ${x.profiles.length} profiles × ${x.repeats} · judged by ${x.judge_name}`),
      e('div', { style: bar },
        x.done ? e(Badge, { variant: 'success' }, 'done') : e(Badge, null, `${finished} of ${runs} solves`),
        !x.done ? e(Button, { size: 'xs', variant: 'outline', disabled: cancel.isPending, onClick: () => cancel.mutate({ experiment: x.id }) }, 'Stop') : null),
      e(Sub, null, 'Ranking'),
      e('div', { style: faint }, 'Most feasible solves first, then the best average place across datasets (each dataset ranks the profiles by their best plan under the judge).'),
      e(Table, {
        columns: [{ id: 'place', label: '#', width: 32 }, { id: 'profile', label: 'Profile' }, { id: 'feasible', label: 'Feasible', num: true }, { id: 'avg', label: 'Average place', num: true }],
        rows: x.ranking.map(r => ({
          key: r.profile_id, place: r.place, profile: e('strong', null, r.profile),
          feasible: r.finished ? `${r.feasible} of ${r.finished}` : '—',
          avg: r.average_place == null ? '—' : r.average_place
        }))
      }),
      e(Sub, null, 'By dataset'),
      e(Table, {
        columns: [{ id: 'dataset', label: 'Dataset' }, ...x.profiles.map(p => ({ id: p, label: x.profile_names[p] || p }))],
        rows: x.datasets.map(d => ({
          key: d, dataset: x.dataset_names[d] || d,
          ...Object.fromEntries(x.profiles.map(p => {
            const c = cell(d, p)
            return [p, c && c.best
              ? e('div', { style: rowLink, onClick: () => go({ view: 'run', id: c.best.run_id }) },
                  e(ScoreText, { score: c.best.score }),
                  e('div', { style: faint }, `median ${c.median} · ${c.feasible}/${c.finished} feasible${c.mean_elapsed_ms ? ` · ${(c.mean_elapsed_ms / 1000).toFixed(1)} s` : ''}`))
              : e('span', { style: faint }, c ? `${c.finished}/${c.runs} done` : '—')]
          }))
        }))
      }))
  })
}

/** One KPI over time: a line through every finished solve, oldest on the left. */
export function TrendLine({ kpi, width = 520, height = 64 }) {
  const values = kpi.points.map(p => p[1])
  if (values.length < 2) return e('div', { style: faint }, `${kpi.label}: ${values.length ? `${values[0]} ${kpi.unit || ''}` : 'no data'}`)
  const low = Math.min(...values)
  const high = Math.max(...values)
  const span = high - low || 1
  const pad = 4
  const d = values.map((v, i) => `${i ? 'L' : 'M'}${(pad + (i / (values.length - 1)) * (width - 2 * pad)).toFixed(1)},${(pad + ((high - v) / span) * (height - 2 * pad)).toFixed(1)}`).join('')
  return e('figure', { style: { margin: 0 } },
    e('svg', { role: 'img', 'aria-label': `${kpi.label} over time`, viewBox: `0 0 ${width} ${height}`, width: '100%', height, preserveAspectRatio: 'none', style: { display: 'block' } },
      e('path', { d, fill: 'none', stroke: 'var(--ui-accent)', strokeWidth: 1.5, vectorEffect: 'non-scaling-stroke' })),
    e('figcaption', { style: faint }, `${kpi.label} · ${values.length} solves · latest ${values[values.length - 1].toLocaleString()} ${kpi.unit || ''} · range ${low.toLocaleString()}–${high.toLocaleString()}`))
}

function TrendsView({ model }) {
  const query = useRead('trends', { model })
  return e(Loading, { query }, data => e('div', { style: section },
    e(Title, null, `Trends · ${data.model_id}`),
    data.solves.length
      ? e('div', { style: muted }, `${data.solves.length} finished solves · ${Math.round(data.feasible_share * 100)}% feasible · ${when(data.solves[0].finished_at)} to ${when(data.solves[data.solves.length - 1].finished_at)}`)
      : e(EmptyState, { title: 'No finished solves yet', description: 'Trends appear once this model has solved plans.' }),
    data.kpis.map(k => e(TrendLine, { key: k.id, kpi: k }))))
}

function RunsView({ go }) {
  const query = useRead('runs', {}, { refetchInterval: 3_000 })
  return e(Loading, { query }, data =>
    data.runs.length ? e(RunsTable, { runs: data.runs, go }) : e('div', { style: muted }, 'No runs yet.'))
}

const TABS = [
  { id: 'models', label: 'Models' },
  { id: 'datasets', label: 'Datasets' },
  { id: 'runs', label: 'Runs' },
  { id: 'experiments', label: 'Experiments' }
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
    compare: () => e(CompareView, { ids: here.id }),
    experiments: () => e(ExperimentsView, { go }),
    experiment: () => e(ExperimentView, { id: here.id, go }),
    'experiment-new': () => e(ExperimentNew, { model: here.id, datasets: here.datasets, go }),
    trends: () => e(TrendsView, { model: here.id })
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
  description: 'Optimization models: catalog, datasets, profiles, runs, fill, experiments and their explanations.',
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
