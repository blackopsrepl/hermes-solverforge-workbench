/**
 * SolverForge Workbench — Desktop half.
 *
 * One page in the SolverForge Workbench section: models on the home screen, a hub per
 * model (overview, datasets, runs, profiles, experiments, trends), and every dataset,
 * run, comparison and experiment inside it. The place lives in the page address
 * (`#/solverforge-workbench?at=/m/<model>/runs/<run>`), so back and forward, the palette,
 * the status-bar chip and chat cards all land on exact places, and the last place is
 * shown again when the page is opened from the sidebar. The same page can sit beside the
 * chat.
 *
 * Every byte comes from this package's backend (dashboard/plugin_api.py) through ctx.rest;
 * every write is a button the user presses, and destructive ones ask first.
 */

import {
  Badge,
  Button,
  ConfirmDialog,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  CopyButton,
  EmptyState,
  ErrorState,
  host,
  KEYBINDS_AREA,
  PALETTE_AREA,
  queryClient,
  ROUTES_AREA,
  SandboxedFrame,
  SearchField,
  SIDEBAR_NAV_AREA,
  Skeleton,
  STATUSBAR_AREAS,
  TRANSCRIPT_DIRECTIVE_AREA,
  useMutation,
  useQuery
} from '@hermes/plugin-sdk'
import { createContext, useContext, useEffect, useRef, useState } from 'react'
import { jsx, jsxs } from 'react/jsx-runtime'

const ID = 'solverforge-workbench'
const BASE = '/solverforge-workbench'
const LIVE = ['queued', 'solving']
const FINISHED = ['completed', 'incomplete']

let rest = () => Promise.reject(new Error('SolverForge Workbench not loaded'))
let storage = { get: (_key, fallback) => fallback, set() {} }
// The host's OS and folder doors (save dialogs, the folder picker); absent on older hosts.
let os = null
let files = null

// One element: a single child goes in as is, several as a static list.
// React reads `key` from jsx()'s third argument; inside props it is dropped.
const e = (type, props, ...children) => {
  const kids = children.flat().filter(c => c !== null && c !== undefined && c !== false)
  const { key, ...rest } = props || {}
  if (!kids.length) return jsx(type, rest, key)
  return (kids.length > 1 ? jsxs : jsx)(type, { ...rest, children: kids.length > 1 ? kids : kids[0] }, key)
}

// ---- look ----------------------------------------------------------------------

const tone = (color, size = '0.75rem') => ({ color: `var(${color})`, fontSize: size })
const muted = tone('--ui-text-secondary')
// Small text stays readable on light themes too: between secondary and tertiary, at AA.
const faint = { color: 'color-mix(in srgb, var(--ui-text-secondary) 70%, var(--ui-text-tertiary))', fontSize: '0.7rem' }
// A status colour as text: blended toward the theme's text colour, so it keeps its hue
// and reads at AA on light and dark backgrounds alike.
const ink = token => `color-mix(in srgb, var(${token}) 58%, var(--ui-text-primary))`
const DANGER = ink('--ui-danger, #e5534b')
const stack = (gap = 8) => ({ display: 'flex', flexDirection: 'column', gap })
const row = (gap = 8) => ({ display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap })
const card = { border: '1px solid var(--ui-stroke-secondary)', borderRadius: 10, padding: 14, minWidth: 0 }
const input = {
  background: 'var(--ui-bg-tertiary)', color: 'var(--ui-text-primary)', border: '1px solid var(--ui-stroke-secondary)',
  borderRadius: 5, padding: '3px 7px', fontSize: '0.75rem'
}
const table = { width: '100%', borderCollapse: 'collapse', fontSize: '0.75rem' }
const cell = { padding: '7px 8px', borderBottom: '1px solid var(--ui-stroke-tertiary)', textAlign: 'left', verticalAlign: 'middle' }
const head = { ...cell, ...faint, fontWeight: 600 }
const num = { ...cell, textAlign: 'right', fontVariantNumeric: 'tabular-nums' }
const LEVEL_COLOR = { hard: '--ui-danger, #e5534b', medium: '--ui-warning, #d29922', soft: '--ui-text-secondary' }
const levelColor = level => (LEVEL_COLOR[level] && level !== 'soft' ? ink(LEVEL_COLOR[level]) : 'var(--ui-text-secondary)')
const spacer = () => e('span', { style: { flex: 1 } })

const H1 = ({ children }) => e('h1', { style: { margin: 0, fontSize: '1.15rem', fontWeight: 600 } }, children)
const H2 = ({ children }) => e('h2', { style: { margin: 0, fontSize: '0.9rem', fontWeight: 600 } }, children)
const Label = ({ children }) => e('div', { style: { ...faint, textTransform: 'uppercase', letterSpacing: '0.06em', fontWeight: 600 } }, children)
const Card = ({ children, style }) => e('section', { style: { ...card, ...stack(10), ...style } }, children)

// ---- numbers, scores, time and names -----------------------------------------------

export const fmt = value => (value === null || value === undefined || Number.isNaN(Number(value)) ? '—' : Number(value).toLocaleString('en-US'))

/** "-6hard/0medium/-12soft" -> [{level:'hard', value:-6}, …]. */
export const parseScore = score =>
  String(score || '')
    .split('/')
    .map(part => /^(-?[0-9.]+)([a-z]+)$/i.exec(part.trim()))
    .filter(Boolean)
    .map(m => ({ level: m[2].toLowerCase(), value: Number(m[1]) }))

/** A score per level, in its level's colour, with thousands separators; zero levels faint. */
export function ScoreText({ score }) {
  const parts = parseScore(score)
  if (!parts.length) return e('span', { style: faint }, '—')
  return e('span', { style: { ...row(10), display: 'inline-flex', fontVariantNumeric: 'tabular-nums', fontWeight: 600 } },
    parts.map(p => e('span', { key: p.level, style: p.value ? { color: levelColor(p.level) } : { color: faint.color, fontWeight: 400 } },
      `${fmt(p.value)} ${p.level}`)))
}

/** The sum of a score's hard level when it is below zero; 0 means no hard rule is broken. */
export const brokenHard = score => parseScore(score).filter(p => p.level === 'hard' && p.value < 0).reduce((n, p) => n + p.value, 0)

/** What a run's status means to a person, and the colour of its dot. */
export const STATUS = {
  queued: ['Waiting', '--ui-text-tertiary'],
  solving: ['Solving', '--ui-accent'],
  completed: ['Finished', '--ui-success, #3fb950'],
  incomplete: ['Stopped early', '--ui-warning, #d29922'],
  failed: ['Failed', '--ui-danger, #e5534b'],
  invalid: ['Data problem', '--ui-danger, #e5534b']
}
function Dot({ status }) {
  const color = (STATUS[status] || [null, '--ui-text-tertiary'])[1]
  return e('span', { 'aria-hidden': true, style: { width: 7, height: 7, borderRadius: 4, background: `var(${color})`, display: 'inline-block', flex: 'none',
    boxShadow: status === 'solving' ? `0 0 0 3px color-mix(in srgb, var(${color}) 25%, transparent)` : 'none' } })
}
export function Status({ status }) {
  return e('span', { style: { ...row(6), display: 'inline-flex', whiteSpace: 'nowrap' } }, e(Dot, { status }), (STATUS[status] || [status || '—'])[0])
}

const pad2 = n => String(n).padStart(2, '0')
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
const clock = date => `${pad2(date.getHours())}:${pad2(date.getMinutes())}`
const sameDay = (a, b) => a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
const day = date => `${date.getDate()} ${MONTHS[date.getMonth()]}`

/** "just now", "12 min ago", "3 h ago", "yesterday 14:05", "3 Nov 14:05". */
export function ago(stamp, now = new Date()) {
  if (!stamp) return ''
  const then = new Date(stamp)
  if (Number.isNaN(then.getTime())) return ''
  const minutes = Math.round((now - then) / 60_000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes} min ago`
  if (minutes < 6 * 60 && sameDay(then, now)) return `${Math.round(minutes / 60)} h ago`
  if (sameDay(then, now)) return `today ${clock(then)}`
  const yesterday = new Date(now)
  yesterday.setDate(now.getDate() - 1)
  if (sameDay(then, yesterday)) return `yesterday ${clock(then)}`
  return `${day(then)} ${clock(then)}`
}

/** A run's name: the one it was given, else its kind and start time ("Run 19:56"). */
export function runName(run, now = new Date()) {
  if (!run) return ''
  if (run.label) return run.label
  const word = run.kind === 'evaluate' ? 'Check' : 'Run'
  const when = new Date(run.created_at)
  if (Number.isNaN(when.getTime())) return word
  return sameDay(when, now) ? `${word} ${clock(when)}` : `${word} ${day(when)} ${clock(when)}`
}

/** "45 s", "4:58", "1 h 02 min". */
export function duration(ms) {
  if (ms === null || ms === undefined) return '—'
  const s = Math.round(ms / 1000)
  if (s < 60) return `${s} s`
  if (s < 3600) return `${Math.floor(s / 60)}:${pad2(s % 60)}`
  return `${Math.floor(s / 3600)} h ${pad2(Math.floor((s % 3600) / 60))} min`
}

/** "1 run", "3 runs"; nouns ending in y take -ies. */
export const plural = noun => (noun.endsWith('y') ? noun.slice(0, -1) + 'ies' : noun + 's')
export const count = (n, noun) => `${fmt(n)} ${n === 1 ? noun : plural(noun)}`

export const sizeText = sizes => (Array.isArray(sizes) ? sizes : []).map(s => `${fmt(s.count)} ${s.count === 1 && s.one ? s.one : s.label}`).join(' · ')

/** Where a dataset came from, in words. */
// Which travel a plan runs on.
const TRAVEL = {
  roads: 'travel along the roads (OpenStreetMap)',
  dataset: 'travel times from the dataset',
}

export function madeFrom(dataset) {
  const parent = dataset.parent_name || 'another dataset'
  return {
    demo: 'Demo',
    upload: 'Imported',
    input: 'Entered',
    patch: `Edited from ${parent}`,
    run: `Replan of ${parent}`,
    fill: `Fill on ${parent}`,
    copy: `Copy of ${parent}`
  }[dataset.origin] || dataset.origin
}

/** The profile a run was solved under; runs keep their results when their profile is deleted. */
export const profileLabel = run => run.profile_name || (run.profile_id && !String(run.profile_id).endsWith('-standard') ? 'A deleted profile' : 'Standard')

/** One sentence per refusal code; anything else keeps the backend's own message. */
const ERRORS = {
  engine_missing: 'The SolverForge engine is not installed. Type /solverforge-workbench install in the chat.',
  busy: 'Something is still solving here. Stop it first.',
  stale: 'This changed somewhere else since you opened it. It has been reloaded; make your change again.',
  unknown_model: 'This model is no longer installed.',
  unknown_dataset: 'This dataset no longer exists.',
  unknown_run: 'This run no longer exists.',
  unknown_profile: 'This profile no longer exists.',
  unknown_experiment: 'This experiment no longer exists.',
  name_taken: 'A profile with this name already exists.',
  read_only: 'Standard cannot be changed. Save your changes as a new profile.',
  weights_unsupported: 'This model has fixed weights; only its settings can change.',
  too_many_runs: 'That is more than 200 solves. Pick fewer datasets, profiles or repeats.',
  finished: 'This run has already finished.',
  no_plan: 'There is no plan here yet.',
  no_view: 'This model does not draw its plans.',
  storage_error: 'The workbench could not read its data folder.',
  confirm_required: 'Confirm the deletion in its dialog.',
  model_busy: 'This model is still solving. Stop its solves first.',
  stock_model: 'Stock models cannot be removed.',
  not_a_project: 'That folder is not a solverforge-cli project: it has no solverforge.app.toml.',
  cargo_missing: 'Building your model needs a Rust toolchain. Install rustup (rustup.rs) and try again.',
  id_taken: 'Another model already has this id.',
  invalid_profile: 'That file is not an exported profile.',
  invalid_patch: 'That change does not fit this dataset.'
}
export function errorText(result, fallback = 'Something went wrong.') {
  if (!result) return fallback
  if (ERRORS[result.code]) return ERRORS[result.code]
  const message = String(result.error || result.message || fallback)
  return message.charAt(0).toUpperCase() + message.slice(1).replace(/\.?$/, '.')
}

// ---- search ------------------------------------------------------------------------

const WORD_START = /[\s\-_/·:(,]/

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

/** Every whitespace-separated term must match some field ([key or getter, weight]; the
 *  first field is the one highlighted). Results keep the input order when the query is
 *  empty and rank best first otherwise; `hits` index into the first field. */
export function searchItems(items, query, fields) {
  const terms = String(query || '').trim().split(/\s+/).filter(Boolean)
  if (!terms.length) return items.map(item => ({ item, score: 0, hits: [] }))
  const main = fields[0][0]
  const found = []
  for (const item of items) {
    let total = 0
    const hits = new Set()
    let matched = true
    let previous = -1 // last main-field position matched by the term before, to reward terms read in order
    for (const term of terms) {
      let best = null
      for (const [field, weight] of fields) {
        const m = fuzzyMatch(term, typeof field === 'function' ? field(item) : item[field])
        if (m && (!best || m.score * weight > best.score)) best = { score: m.score * weight, field, hits: m.hits }
      }
      if (!best) { matched = false; break }
      total += best.score
      if (best.field === main) {
        if (best.hits[0] > previous && previous >= 0) total += 8
        previous = best.hits[best.hits.length - 1]
        best.hits.forEach(h => hits.add(h))
      }
    }
    if (matched) found.push({ item, score: total, hits: [...hits].sort((x, y) => x - y) })
  }
  return found.sort((x, y) => y.score - x.score)
}

const CONSTRAINT_FIELDS = [['name', 1], ['group', 0.6], ['level', 0.6], ['description', 0.4]]
export const searchConstraints = (items, query) => searchItems(items, query, CONSTRAINT_FIELDS)

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
  Array.from(String(text)).forEach((ch, i) => {
    if (marks.has(i) !== marked) { flush(); marked = marks.has(i) }
    run += ch
  })
  flush()
  return e('span', null, ...parts)
}

// ---- data --------------------------------------------------------------------------

const qk = (...parts) => [ID, ...parts]
const param = value => encodeURIComponent(value)

const useRead = (kind, query = {}, options = {}) => {
  const search = Object.entries(query).filter(([, v]) => v !== undefined && v !== null && v !== '')
  const path = `/${kind}${search.length ? '?' + search.map(([k, v]) => `${k}=${param(v)}`).join('&') : ''}`
  return useQuery({ queryKey: qk(kind, ...search.flat()), queryFn: () => rest(path), staleTime: 3_000, ...options })
}
const answer = (query, key) => (query.data && query.data.ok ? query.data[key] : null)

function useAction(action, onDone) {
  return useMutation({
    mutationFn: body => rest(`/actions/${action}`, { method: 'POST', body, timeoutMs: 120_000 }),
    onSuccess: result => {
      queryClient.invalidateQueries({ queryKey: [ID] })
      if (result && result.ok === false) host.notify({ kind: 'error', message: errorText(result) })
      else if (onDone) onDone(result)
    },
    onError: error => host.notify({ kind: 'error', message: errorText({ error: error && error.message }, 'The SolverForge Workbench did not answer.') })
  })
}

const profileName = () => (host.state && host.state.profile && typeof host.state.profile.get === 'function' && host.state.profile.get()) || 'default'

/** A value kept across visits (search text, filters, the last place), per Hermes profile. */
function useStored(key, initial) {
  const full = `${profileName()}:${key}`
  const [value, setValue] = useState(() => storage.get(full, initial))
  return [value, next => { setValue(next); storage.set(full, next) }]
}

// ---- states ------------------------------------------------------------------------

function Loading({ query, children, lines = 3 }) {
  if (query.isLoading) {
    return e('div', { style: stack(8), 'aria-busy': true },
      Array.from({ length: lines }, (_, i) => e(Skeleton, { key: i, style: { height: 18, width: `${90 - i * 15}%` } })))
  }
  if (!query.data || query.data.ok === false) {
    return e(ErrorState, {
      title: errorText(query.data, 'The SolverForge Workbench did not answer.'),
      children: e(Button, { size: 'xs', variant: 'outline', onClick: () => query.refetch && query.refetch() }, 'Try again')
    })
  }
  return children(query.data)
}

/** An empty list: what it will hold and the button that starts it. */
const Empty = ({ title, description, action }) =>
  e('div', { style: { ...stack(10), alignItems: 'flex-start', padding: '8px 0' } },
    e(EmptyState, { title, description }), action || null)

// ---- the page address --------------------------------------------------------------

/** The page address of a place in the workbench: `/solverforge-workbench?at=<place>`. */
export const hrefOf = at => `${BASE}?at=${encodeURIComponent(at)}`

/** The place in the current page address, or '' on the bare section address. */
function readAt() {
  if (typeof window === 'undefined') return ''
  const hash = window.location.hash || ''
  const q = hash.indexOf('?')
  if (!hash.startsWith(`#${BASE}`) || q < 0) return ''
  return new URLSearchParams(hash.slice(q + 1)).get('at') || ''
}

/** A place as its parts: '/m/field-service-routing/runs/abc' -> { page:'model', model, tab:'runs', id:'abc' }. */
export function parseAt(at) {
  const [first, ...more] = String(at || '').split('/').filter(Boolean).map(decodeURIComponent)
  if (first === 'm' && more[0]) {
    const [model, tab = 'overview', id, extra, also] = more
    return { page: 'model', model, tab, id, extra: extra === '-' ? undefined : extra, ...(also ? { also } : {}) }
  }
  if (first === 'runs' || first === 'experiments') return { page: first }
  if (first === 'add-model') return { page: 'add' }
  if (first === 'run' || first === 'dataset' || first === 'experiment') return { page: 'find', kind: first, id: more[0] }
  return { page: 'home' }
}
export const placeOf = (model, tab, id, extra, also) =>
  `/m/${[model, tab, id, extra || (also ? '-' : ''), also].filter(Boolean).map(encodeURIComponent).join('/')}`

const Nav = createContext({ at: '/home', go: () => {}, docked: false })
const useNav = () => useContext(Nav)

/** A link to a place: a real button, so it reads and focuses as one. */
function Link({ to, children, style }) {
  const { go } = useNav()
  return e('button', {
    type: 'button', onClick: event => { event.stopPropagation(); go(to) },
    style: { background: 'none', border: 0, padding: 0, color: 'inherit', font: 'inherit', cursor: 'pointer', textAlign: 'left', ...style }
  }, children)
}

// ---- keyboard ----------------------------------------------------------------------

// The page's keys act only while the page is on screen and no field has focus. Each screen
// registers what its keys do (n new dataset, s solve, e new experiment); the palette and
// Settings › Keyboard reach the same actions.
const keyOwners = new Map()
let nextOwner = 0
function useKeys(actions) {
  const owner = useRef(null)
  if (owner.current === null) owner.current = ++nextOwner
  keyOwners.set(owner.current, actions)
  useEffect(() => () => { keyOwners.delete(owner.current) }, [])
}
export function runKey(key) {
  const owners = [...keyOwners.keys()].sort((a, b) => b - a)
  for (const owner of owners) {
    const action = keyOwners.get(owner)[key]
    if (action) { action(); return true }
  }
  return false
}
function focusSearch() {
  const field = typeof document !== 'undefined' && document.querySelector('[data-wb-search] input')
  if (field) field.focus()
}

// ---- shared pieces -----------------------------------------------------------------

function Crumbs({ items }) {
  return e('nav', { 'aria-label': 'Where you are', style: { ...row(6), ...muted } },
    items.map((item, i) => e('span', { key: i, style: row(6) },
      i ? e('span', { 'aria-hidden': true, style: faint }, '›') : null,
      i === items.length - 1 || !item.to
        ? e('span', { style: i === items.length - 1 ? { color: 'var(--ui-text-primary)', fontWeight: 600 } : undefined }, item.label)
        : e(Link, { to: item.to }, item.label))))
}

function TabBar({ tabs, value, onChange }) {
  return e('div', { role: 'tablist', style: { display: 'flex', gap: 2, borderBottom: '1px solid var(--ui-stroke-tertiary)', overflowX: 'auto' } },
    tabs.map(t => e('button', {
      key: t.id, type: 'button', role: 'tab', 'aria-selected': t.id === value, onClick: () => onChange(t.id),
      style: { background: 'none', border: 0, borderBottom: `2px solid ${t.id === value ? 'var(--ui-accent)' : 'transparent'}`,
        marginBottom: -1, padding: '7px 12px', cursor: 'pointer', fontSize: '0.78rem', whiteSpace: 'nowrap',
        color: t.id === value ? 'var(--ui-text-primary)' : faint.color }
    }, t.label, t.count !== undefined && t.count !== null ? e('span', { style: { ...faint, marginLeft: 5 } }, fmt(t.count)) : null)))
}

/** A search field over a list with "N of M", cleared by Escape; `/` focuses it. */
function ListSearch({ value, onChange, shown, total, noun, placeholder, children }) {
  // The search field draws no focus ring of its own; mark the focused field here.
  const [focused, setFocused] = useState(false)
  return e('div', { style: { ...row(8), padding: '2px 0' } },
    e('div', {
      'data-wb-search': true, onFocus: () => setFocused(true), onBlur: () => setFocused(false),
      style: { flex: '1 1 260px', minWidth: 200, borderRadius: 8, outline: focused ? '2px solid var(--ui-accent)' : 'none', outlineOffset: 1 }
    },
      e(SearchField, {
        placeholder: placeholder || `Search ${plural(noun)}`, value, onChange, onClear: () => onChange(''), variant: 'box',
        onKeyDown: event => { if (event.key === 'Escape') onChange('') }
      })),
    e('span', { style: faint, 'aria-live': 'polite' }, value.trim() ? `${fmt(shown)} of ${fmt(total)}` : count(total, noun)),
    children || null)
}

/** A small select that reads as a filter: "Status any". */
function Pick({ label, value, onChange, options }) {
  return e('label', { style: { ...row(4), ...faint } }, label,
    e('select', { value, 'aria-label': label, style: input, onChange: event => onChange(event.target.value) },
      options.map(([v, text]) => e('option', { key: v, value: v }, text))))
}

function Table({ columns, rows, onRow, empty }) {
  if (!rows.length && empty) return empty
  const fixed = columns.some(c => c.width)
  return e('table', { style: fixed ? { ...table, tableLayout: 'fixed' } : table },
    e('thead', null, e('tr', null, columns.map(c => e('th', { key: c.id, style: { ...(c.num ? { ...head, textAlign: 'right' } : head), ...(c.width ? { width: c.width } : {}) } }, c.label)))),
    e('tbody', null, rows.map(r =>
      e('tr', onRow
        ? { key: r.key, style: { cursor: 'pointer' }, tabIndex: 0, onClick: () => onRow(r),
            // Enter or Space on a focused row opens it, as a click does; keys pressed in a
            // control inside the row (a checkbox) stay with that control.
            onKeyDown: event => { if ((event.key === 'Enter' || event.key === ' ') && event.target === event.currentTarget) { event.preventDefault(); onRow(r) } } }
        : { key: r.key },
        columns.map(c => e('td', { key: c.id, style: c.num ? num : cell }, r[c.id]))))))
}

/** An id kept out of sight: a quiet copy button. */
const CopyId = ({ id, what }) => e(CopyButton, { text: id, label: `Copy the ${what}'s id`, title: `Copy the ${what}'s id`, appearance: 'icon', buttonSize: 'icon-xs', buttonVariant: 'ghost' })

/** Delete, asked first in a dialog that says what goes with it (the agent's approval sentence). */
function DeleteButton({ action, args, what, onDone, label = 'Delete' }) {
  const [open, setOpen] = useState(false)
  const text = useRead('confirm', { action, ...args }, { enabled: open })
  const remove = useAction(action, result => { setOpen(false); if (onDone) onDone(result) })
  return e('span', null,
    e(Button, { size: 'xs', variant: 'text', onClick: () => setOpen(true) }, label),
    e(ConfirmDialog, {
      open, onClose: () => setOpen(false), destructive: true, confirmLabel: label, busyLabel: 'Deleting…',
      title: `${label} ${what}?`,
      description: (text.data && text.data.ok && text.data.text) || ' ',
      onConfirm: () => remove.mutateAsync({ ...args, confirm: true })
    }))
}

const fileName = name => String(name || 'export').replace(/[^\w .-]+/g, '').trim().replace(/\s+/g, '-').toLowerCase() || 'export'

/** Saves JSON the backend writes (a dataset, a run's plan, a profile) where the user picks. */
async function exportJson(action, args, name) {
  if (!os || typeof os.pickSavePath !== 'function') {
    host.notify({ kind: 'error', message: 'This Hermes Desktop has no save dialog. Ask Hermes to export it instead.' })
    return
  }
  const path = await os.pickSavePath({ defaultPath: `${fileName(name)}.json`, title: 'Export', filters: [{ name: 'JSON', extensions: ['json'] }] })
  if (!path) return
  const result = await rest(`/actions/${action}`, { method: 'POST', body: { ...args, path: path.endsWith('.json') ? path : `${path}.json` } })
    .catch(error => ({ ok: false, error: error && error.message }))
  host.notify(result && result.ok ? { kind: 'success', message: `Saved to ${result.path}` } : { kind: 'error', message: errorText(result) })
}

const ExportButton = ({ action, args, name, label = 'Export' }) =>
  e(Button, { size: 'xs', variant: 'text', onClick: () => exportJson(action, args, name) }, label)

/** A button that opens a JSON file and hands its name and parsed contents on. */
function JsonFileButton({ label, onFile, variant = 'outline' }) {
  const field = useRef(null)
  return e('span', null,
    e(Button, { size: 'xs', variant, onClick: () => field.current && field.current.click() }, label),
    e('input', {
      ref: field, type: 'file', accept: '.json,application/json', style: { display: 'none' }, 'aria-label': label,
      onChange: async event => {
        const file = event.target.files && event.target.files[0]
        event.target.value = ''
        if (!file) return
        try {
          onFile({ name: file.name.replace(/\.json$/i, ''), value: JSON.parse(await file.text()) })
        } catch {
          onFile({ name: file.name, error: `${file.name} is not valid JSON.` })
        }
      }
    }))
}

// A chart's caption: the start and end stay at the ends, the words between wrap.
const CAPTION = { ...faint, display: 'grid', gridTemplateColumns: 'auto 1fr auto', gap: 12, textAlign: 'center' }

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
  const padding = 6
  const band = height / lines.length
  return e('figure', { style: { margin: 0 } },
    e('svg', { role: 'img', 'aria-label': 'Score over time', viewBox: `0 0 ${width} ${height}`, width: '100%', height, preserveAspectRatio: 'none', style: { display: 'block' } },
      lines.map(([level, values], n) => {
        const low = Math.min(...values.map(([, v]) => v))
        const high = Math.max(...values.map(([, v]) => v))
        const span = high - low || 1
        const top = n * band
        const d = values
          .map(([t, v], i) => `${i ? 'L' : 'M'}${(padding + (t / end) * (width - 2 * padding)).toFixed(1)},${(top + padding + ((high - v) / span) * (band - 2 * padding)).toFixed(1)}`)
          .join('')
        return e('g', { key: level },
          n ? e('line', { x1: 0, x2: width, y1: top, y2: top, stroke: 'var(--ui-stroke-tertiary)', strokeWidth: 1, vectorEffect: 'non-scaling-stroke' }) : null,
          e('path', { d, fill: 'none', stroke: levelColor(level), strokeWidth: 1.5, vectorEffect: 'non-scaling-stroke' }))
      })),
    e('figcaption', { style: CAPTION },
      e('span', null, '0:00'),
      e('span', null, `Score over time · top to bottom: ${lines.map(([level]) => level).join(', ')}, each on its own scale`),
      e('span', null, duration(end))))
}

/** What a model's plan view is called: a schedule, routes on a map, or boards of assignments. */
export function viewName(kind) {
  return kind === 'routes' ? 'Routes' : kind === 'assignments' ? 'Assignments' : 'Schedule'
}

/** The plan as its model draws it (solverforge-ui's timeline, routes on a map, or boards)
 *  in a sandboxed frame. */
export function PlanView({ run, dataset, version, kind = 'timeline', onItem }) {
  const query = useRead('view', { run, dataset, v: version })
  const frame = useRef(null)
  // A click on an open item inside the frame (it posts {source, item}) asks for its places.
  useEffect(() => {
    if (!onItem || typeof window === 'undefined') return undefined
    const listen = event => {
      const data = event.data
      if (frame.current && event.source === frame.current.contentWindow && data && data.source === ID && data.item && data.item.id) {
        onItem({ id: String(data.item.id), label: String(data.item.label || data.item.id) })
      }
    }
    window.addEventListener('message', listen)
    return () => window.removeEventListener('message', listen)
  }, [onItem])
  const data = query.data
  const what = viewName(kind).toLowerCase()
  if (query.isLoading) return e(Skeleton, { style: { height: kind === 'routes' ? 560 : 320, width: '100%' } })
  if (!data || data.ok === false) return e('div', { style: faint }, errorText(data, `The ${what} could not be drawn.`))
  // A backend from before the split answers a routes view as one page: draw that.
  if (data.kind === 'routes' && Array.isArray(data.routes)) return e(RoutesPlan, { data, frame, onItem })
  return e(SandboxedFrame, {
    ref: frame,
    src: 'data:text/html;charset=utf-8,' + encodeURIComponent(data.document),
    sandbox: 'allow-scripts',
    title: viewName(kind),
    style: { width: '100%', height: kind === 'routes' ? 'min(78vh, 1040px)' : 560, border: '1px solid var(--ui-stroke-secondary)', borderRadius: 10, display: 'block', background: '#fff' }
  })
}

// A routing plan: the map fills the space beside the list of routes, and the per-vehicle
// timeline sits below as its own surface. Picking a route in the list focuses it on the map.
function RoutesPlan({ data, frame, onItem }) {
  const { docked: dockedPane } = useNav()
  // Narrow by its own width, not only when docked: a page squeezed beside the chat
  // stacks the map over the list the same way.
  const root = useRef(null)
  const [width, setWidth] = useState(0)
  useEffect(() => {
    if (!root.current || typeof ResizeObserver === 'undefined') return undefined
    const watch = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width))
    watch.observe(root.current)
    return () => watch.disconnect()
  }, [])
  const docked = dockedPane || (width > 0 && width < 860)
  const [focus, setFocus] = useState(null)
  const [search, setSearch] = useState('')
  const show = id => {
    const next = focus === id ? null : id
    setFocus(next)
    const win = frame.current && frame.current.contentWindow
    if (win) win.postMessage({ source: ID, focus: next }, '*')
  }
  const found = searchItems(data.routes, search, [['label', 1], [r => r.stats.join(' '), 0.4]])
  const brokenWords = r => [r.broken.hard ? `${count(r.broken.hard, 'stop')} breaking a hard rule` : '',
    r.broken.medium ? `${count(r.broken.medium, 'stop')} breaking a medium rule` : ''].filter(Boolean).join(' · ')
  const height = docked ? '60vh' : 'max(560px, calc(100vh - 220px))'
  const list = e('div', { style: { ...stack(8), minHeight: 0, overflow: docked ? 'visible' : 'auto', paddingRight: docked ? 0 : 4 } },
    // Keyed: the optional "Show every route" row must not shift the list and remount it
    // (that would drop the keyboard focus from the route just picked).
    e(ListSearch, { key: 'search', value: search, onChange: setSearch, shown: found.length, total: data.routes.length, noun: 'route', placeholder: 'Search routes' }),
    focus ? e('div', { key: 'all' }, e(Button, { size: 'xs', variant: 'text', onClick: () => show(focus) }, 'Show every route')) : null,
    e('div', { key: 'list', role: 'list', 'aria-label': 'Routes', style: stack(2) },
      found.map(({ item: r, hits }) => e('button', {
        key: r.id, type: 'button', role: 'listitem', 'aria-pressed': focus === r.id, onClick: () => show(r.id),
        style: { display: 'grid', gridTemplateColumns: '12px 1fr', gap: '2px 8px', alignItems: 'baseline', textAlign: 'left', cursor: 'pointer',
          padding: '6px 8px', borderRadius: 6, font: 'inherit', color: 'inherit',
          border: `1px solid ${focus === r.id ? 'var(--ui-accent)' : 'transparent'}`,
          background: focus === r.id ? 'var(--ui-bg-tertiary)' : 'none', opacity: focus && focus !== r.id ? 0.55 : 1 }
      },
      e('span', { 'aria-hidden': true, style: { width: 12, height: 12, borderRadius: 3, background: r.color, alignSelf: 'center' } }),
      e('span', { style: { fontWeight: 600, fontSize: '0.8rem' } }, e(Highlight, { text: r.label, hits })),
      e('span', null),
      e('span', { style: faint }, r.stats.join(' · ')),
      brokenWords(r) ? e('span', null) : null,
      brokenWords(r) ? e('span', { style: { ...faint, color: DANGER } }, brokenWords(r)) : null))),
    data.unassigned.length
      ? e('div', { key: 'open', style: stack(4) },
          e(Label, null, `${fmt(data.unassigned.length)} unplanned`),
          data.unassigned.map(u => e('div', { key: u.id, style: { ...row(6), justifyContent: 'space-between' } },
            e('span', { style: muted }, u.label),
            onItem ? e(Button, { size: 'xs', variant: 'outline', onClick: () => onItem({ id: String(u.id), label: u.label }) }, 'Find a place') : null)))
      : null)
  return e('div', { ref: root, style: stack(12) },
    data.subtitle ? e('div', { style: muted }, data.subtitle) : null,
    e('div', { style: { display: 'grid', gridTemplateColumns: docked ? '1fr' : 'minmax(0, 1fr) 320px', gap: 12, height: docked ? 'auto' : height } },
      e(SandboxedFrame, {
        ref: frame,
        src: 'data:text/html;charset=utf-8,' + encodeURIComponent(data.map),
        sandbox: 'allow-scripts',
        title: 'Routes',
        style: { width: '100%', height, border: '1px solid var(--ui-stroke-secondary)', borderRadius: 10, display: 'block', background: '#fff' }
      }),
      list),
    data.timeline
      ? e('div', { style: stack(6) },
          e(Label, null, 'Day by day'),
          e(SandboxedFrame, {
            src: 'data:text/html;charset=utf-8,' + encodeURIComponent(data.timeline),
            sandbox: 'allow-scripts',
            title: 'Routes day by day',
            style: { width: '100%', height: 120 + 34 * Math.max(1, data.lanes), border: '1px solid var(--ui-stroke-secondary)', borderRadius: 10, display: 'block', background: '#fff' }
          }))
      : null)
}

function Matches({ item }) {
  if (!item.matches || !item.matches.length) {
    // Range, balance and window rules are scored as one total per employee or period, not per match.
    const scored = parseScore(item.score).some(p => p.value !== 0)
    return e('div', { style: faint }, item.match_count ? 'Matches are counted, not listed, for this rule.' : scored ? 'Scored as a total; this rule has no individual matches.' : 'No matches.')
  }
  return e('ul', { style: { margin: '4px 0', paddingLeft: 18, overflowWrap: 'anywhere', ...muted } },
    item.matches.map((m, i) => e('li', { key: i },
      e(ScoreText, { score: m.score }), ' · ', m.entities.map(x => x.label).join(' · '))),
    item.match_count > item.matches.length ? e('li', { style: faint }, `… ${fmt(item.match_count - item.matches.length)} more`) : null)
}

/** Fill: the best places for a plan's open items, ranked by the score each reaches; one click applies one. */
export function FillPanel({ run, dataset, profile, model, open = false, items, onClear }) {
  const { go } = useNav()
  const [asked, setAsked] = useState(open)
  const query = useRead('fill', { run, dataset, profile, top: 3, items: items && items.length ? items.join(',') : undefined }, { enabled: asked || Boolean(items && items.length) })
  const apply = useAction('fill_apply', r => r.dataset && go(placeOf(model || r.dataset.model_id, 'datasets', r.dataset.id)))
  if (!asked && !(items && items.length)) {
    return e('div', null, e(Button, { size: 'xs', variant: 'outline', onClick: () => setAsked(true) }, 'Find places for open items'))
  }
  return e(Loading, { query }, data => {
    if (!data.items.length) return e('div', { style: muted }, 'Nothing is open in this plan.')
    return e('div', { style: stack(10) },
      e('div', { style: row(8) }, e(H2, null, items && items.length ? `Places for ${data.items.map(i => i.label).join(', ')}` : `Fill · ${fmt(data.items.length)} open`),
        items && items.length && onClear ? e(Button, { size: 'xs', variant: 'text', onClick: () => { setAsked(true); onClear() } }, 'Every open item') : null),
      e('div', { style: faint }, 'Each option is scored with the item placed; applying one makes a new dataset.'),
      data.items.map(item => e('div', { key: `${item.type}:${item.id}`, style: stack(6) },
        e('div', null, e('strong', null, item.label)),
        item.options.length
          ? e(Table, {
              columns: [{ id: 'place', label: 'Place' }, { id: 'delta', label: 'Change', num: true, width: 220 }, { id: 'why', label: 'Rules it moves' }, { id: 'act', label: '', width: 72 }],
              rows: item.options.map(option => ({
                key: option.id || option.label,
                place: e('div', null, option.label, option.feasible ? null : e('span', { style: faint }, ' · breaks a hard rule')),
                delta: e(ScoreText, { score: option.delta }),
                why: e('span', { style: faint }, option.changes.map(c => `${c.constraint} ${c.delta}`).join(' · ') || 'no rule changes'),
                act: e(Button, { size: 'xs', disabled: apply.isPending,
                  onClick: () => apply.mutate({ ...(run ? { run } : { dataset }), item: item.id, value: option.id, ...(profile ? { profile } : {}) }) }, 'Apply')
              }))
            })
          : e('div', { style: faint }, 'No place fits this item.'))))
  })
}

/** Whether a solved plan keeps every hard rule, in a badge. */
function Verdict({ run }) {
  if (!run) return e('span', { style: { ...row(6), ...muted } }, e(Dot, { status: 'queued' }), 'Not solved yet')
  const hard = brokenHard(run.score)
  return hard < 0
    ? e(Badge, { variant: 'destructive' }, `Breaks hard rules · ${fmt(hard)} hard`)
    : e(Badge, { variant: 'success' }, 'Feasible')
}

/** b − a per score level: [{level, value}], positive where b is better. */
export function scoreDelta(a, b) {
  const before = new Map(parseScore(a).map(p => [p.level, p.value]))
  return parseScore(b).map(p => ({ level: p.level, value: p.value - (before.get(p.level) || 0) }))
}

/** A score change in words: "+4 hard · −1,200 soft", or "no change". */
export function DeltaText({ from, to }) {
  const parts = scoreDelta(from, to).filter(p => p.value !== 0)
  if (!parts.length) return e('span', { style: faint }, 'no change')
  return e('span', { style: { ...row(8), display: 'inline-flex', fontVariantNumeric: 'tabular-nums', fontWeight: 600 } },
    parts.map(p => e('span', { key: p.level, style: { color: p.value > 0 ? ink('--ui-success, #3fb950') : levelColor(p.level) } },
      `${p.value > 0 ? '+' : '−'}${fmt(Math.abs(p.value))} ${p.level}`)))
}

/** The first line of a run: whether it keeps every hard rule, in a sentence. */
export function verdictText(run) {
  if (LIVE.includes(run.status)) return run.status === 'queued' ? 'Waiting to solve' : 'Solving'
  if (run.status === 'failed' || run.status === 'invalid') return run.status === 'invalid' ? 'The data has a problem' : 'The solve failed'
  if (!run.score) return 'No plan'
  // Name the rules that cost the level, with how often each is broken, when the analysis is there.
  const broken = level => (run.analysis || [])
    .filter(c => c.level === level && parseScore(c.score).some(p => p.level === level && p.value < 0))
    .map(c => `${c.name}${c.match_count ? ` ×${fmt(c.match_count)}` : ''}`)
  const hard = brokenHard(run.score)
  const medium = parseScore(run.score).filter(p => p.level === 'medium' && p.value < 0).reduce((n, p) => n + p.value, 0)
  if (hard < 0) {
    const rules = broken('hard')
    return rules.length ? `Breaks ${rules.length === 1 ? 'a hard rule' : `${rules.length} hard rules`}: ${rules.join(', ')}` : `Breaks hard rules (${fmt(hard)} hard)`
  }
  if (medium < 0) {
    const rules = broken('medium')
    return rules.length ? `Keeps every hard rule; falls short on ${rules.join(', ')}` : `Keeps every hard rule; leaves ${fmt(-medium)} medium unmet`
  }
  return 'Keeps every hard rule'
}

const ORIGIN_WORD = { demo: 'demo', upload: 'imported', run: 'replan', fill: 'fill', patch: 'edited', copy: 'copy' }

/** Where a dataset comes from, root first, and what was made from it, as links. */
export function LineageStrip({ model, lineage, here }) {
  if (!lineage || (!lineage.ancestors.length && !lineage.children.length)) return null
  const link = d => e(Link, { key: d.id, to: placeOf(model, 'datasets', d.id), style: { textDecoration: 'underline' } }, d.name)
  return e('nav', { 'aria-label': 'Where this dataset comes from', style: { ...row(6), ...faint } },
    lineage.ancestors.length ? e('span', null, 'From') : null,
    lineage.ancestors.flatMap((d, i) => [i ? e('span', { key: `s${i}` }, '›') : null, link(d)]),
    lineage.ancestors.length ? e('span', null, '›') : null,
    e('strong', null, here),
    lineage.children.length
      ? e('span', { style: row(6) }, `· ${count(lineage.children.length, 'dataset')} made from it:`,
          lineage.children.flatMap((d, i) => [i ? e('span', { key: `c${i}` }, '·') : null, link(d), e('span', { key: `o${i}` }, `(${ORIGIN_WORD[d.origin] || d.origin})`)]))
      : null)
}

/** What a solve kept and moved against the run it replans, or the solve before it. */
export function ChangesCard({ run }) {
  const [shown, setShown] = useState(false)
  const query = useRead('changes', { run: run.id })
  const changes = answer(query, 'changes')
  if (!changes || !changes.against) return null
  const parts = [[changes.stayed, 'stayed'], [changes.moved.length, 'moved'], [changes.added.length, 'new'],
    [changes.opened.length, 'left open'], [changes.removed.length, 'removed']].filter(([n]) => n)
  const listed = [...changes.moved.map(c => [c, `${c.from} → ${c.to}`]), ...changes.added.map(c => [c, `new · ${c.to || 'open'}`]),
    ...changes.opened.map(c => [c, `${c.from} → open`]), ...changes.removed.map(c => [c, `removed · was ${c.from || 'open'}`])]
  return e(Card, null,
    e('div', { style: row(8) }, e(Label, null, 'What changed'), spacer(),
      e('span', { style: faint }, 'against ', e(Link, { to: placeOf(run.model_id, 'runs', changes.against.id), style: { textDecoration: 'underline' } }, runName(changes.against)))),
    e('div', { style: row(10) },
      e(DeltaText, { from: changes.against.score, to: run.score }),
      e('span', { style: muted }, parts.map(([n, word]) => `${fmt(n)} ${word}`).join(' · ') || 'nothing to compare')),
    listed.length
      ? e('div', null,
          e(Button, { size: 'xs', variant: 'text', onClick: () => setShown(!shown) }, shown ? 'Hide what moved' : `Show what moved (${fmt(listed.length)})`),
          shown ? e('ul', { style: { margin: '4px 0', paddingLeft: 18, ...muted } },
            listed.slice(0, 200).map(([c, words]) => e('li', { key: c.id }, e('strong', null, c.label), ` · ${words}`))) : null)
      : null)
}

/** One field of a replanning change, edited as its kind says. */
function EditField({ field, value, onChange }) {
  const id = `edit-${field.name}`
  const control = field.kind === 'choice'
    ? e('select', { id, value: value || '', style: input, onChange: event => onChange(event.target.value) },
        e('option', { value: '' }, field.optional ? '(none)' : 'Choose…'),
        field.options.map(o => e('option', { key: o.value, value: o.value }, o.label)))
    : field.kind === 'minutes'
      ? e('input', { id, type: 'number', min: 1, value: value || '', style: { ...input, width: 90 }, onChange: event => onChange(event.target.value ? Math.trunc(Number(event.target.value)) : '') })
      : field.kind === 'date_time'
        ? e('input', { id, type: 'datetime-local', value: value || '', style: input, onChange: event => onChange(event.target.value.slice(0, 16)) })
        : field.kind === 'location'
          ? e('input', { id, placeholder: 'latitude, longitude', value: value || '', style: { ...input, width: 180 },
              onChange: event => onChange(event.target.value) })
          : e('input', { id, value: value || '', style: { ...input, width: 220 }, onChange: event => onChange(event.target.value) })
  return e('label', { htmlFor: id, style: { ...row(6), ...muted } }, e('span', { style: { minWidth: 110 } }, field.label), control)
}

/** "45.07, 7.68" -> [45.07, 7.68]; anything else stays text for the engine to refuse in words. */
export function editArgs(edit, values) {
  const args = {}
  for (const field of edit.fields) {
    const value = values[field.name]
    if (value === undefined || value === '') continue
    if (field.kind === 'location') {
      const parts = String(value).split(',').map(x => Number(x.trim()))
      args[field.name] = parts.length === 2 && parts.every(Number.isFinite) ? parts : value
    } else args[field.name] = value
  }
  return args
}

/** Replanning in one form: the model's changes, what to keep, the profile and the time;
 *  it makes the new dataset from the run (or the dataset) and solves it. */
export function ReplanForm({ model, run, dataset, onDone }) {
  const { go } = useNav()
  const source = run ? { run: run.id } : { dataset: dataset.id }
  const datasetId = run ? run.dataset_id : dataset.id
  const query = useRead('edits', source)
  const offered = answer(query, 'edits') || []
  const [changes, setChanges] = useState([])
  const [adding, setAdding] = useState('')
  const [freeze, setFreeze] = useState('')
  const [profile, setProfile] = useState('')
  const [seconds, setSeconds] = useState('')
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState(null)
  const byId = new Map(offered.map(x => [x.id, x]))
  const submit = async () => {
    setBusy(true)
    setFailure(null)
    try {
      const made = await rest('/actions/dataset_patch', { method: 'POST', timeoutMs: 600_000, body: {
        dataset: datasetId, ...(run ? { from_run: run.id } : {}), ...(name.trim() ? { name: name.trim() } : {}),
        edits: changes.map(c => ({ edit: c.edit, args: editArgs(byId.get(c.edit), c.values) })) } })
      if (!made || made.ok === false) { setFailure(errorText(made)); return }
      const solve = { ...(seconds ? { seconds: Number(seconds) } : {}), ...(freeze ? { freeze_before: freeze.slice(0, 16) } : {}) }
      const started = await rest('/actions/solve', { method: 'POST', timeoutMs: 120_000, body: {
        dataset: made.dataset.id, ...(profile ? { profile } : {}), ...(Object.keys(solve).length ? { overrides: { solve } } : {}) } })
      queryClient.invalidateQueries({ queryKey: [ID] })
      if (!started || started.ok === false) { setFailure(errorText(started)); go(placeOf(model.id, 'datasets', made.dataset.id)); return }
      if (onDone) onDone()
      go(placeOf(model.id, 'runs', started.run.id))
    } catch (error) {
      setFailure(errorText({ error: error && error.message }))
    } finally {
      setBusy(false)
    }
  }
  return e('div', { style: stack(12) },
    e('div', { style: stack(8) },
      e(Label, null, 'Changes'),
      changes.length ? null : e('div', { style: faint }, offered.length ? 'Nothing changes yet: add what happened, or replan as it is.' : 'This model offers no changes; replan keeps the plan as it is.'),
      changes.map((change, n) => {
        const edit = byId.get(change.edit)
        return e(Card, { key: n, style: { padding: 10 } },
          e('div', { style: row(8) }, e('strong', null, edit ? edit.label : change.edit), spacer(),
            e(Button, { size: 'xs', variant: 'text', onClick: () => setChanges(changes.filter((_, i) => i !== n)) }, 'Remove')),
          edit ? e('div', { style: faint }, edit.description) : null,
          edit ? edit.fields.map(field => e(EditField, { key: field.name, field, value: change.values[field.name],
            onChange: value => setChanges(changes.map((c, i) => i === n ? { ...c, values: { ...c.values, [field.name]: value } } : c)) })) : null)
      }),
      offered.length
        ? e('div', { style: row(6) },
            e('select', { value: adding, 'aria-label': 'Change to add', style: input, onChange: event => setAdding(event.target.value) },
              e('option', { value: '' }, 'Add a change…'), offered.map(x => e('option', { key: x.id, value: x.id }, x.label))),
            e(Button, { size: 'xs', variant: 'outline', disabled: !adding, onClick: () => { setChanges([...changes, { edit: adding, values: {} }]); setAdding('') } }, 'Add'))
        : null),
    e('div', { style: stack(8) },
      e(Label, null, 'Solve'),
      run ? e('label', { style: { ...row(6), ...muted } }, e('span', { style: { minWidth: 110 } }, 'Keep everything before'),
        e('input', { type: 'datetime-local', value: freeze, style: input, 'aria-label': 'Keep everything that starts before', onChange: event => setFreeze(event.target.value) })) : null,
      freeze ? e('div', { style: faint }, `Everything that starts before ${freeze.replace('T', ' ')} keeps its place.`) : null,
      e('div', { style: row(8) },
        e(ProfilePicker, { model, value: profile, onChange: setProfile }),
        e('input', { type: 'number', min: 1, placeholder: 'seconds', value: seconds, style: { ...input, width: 84 }, 'aria-label': 'Time limit in seconds',
          onChange: event => setSeconds(event.target.value ? String(Math.max(1, Math.trunc(Number(event.target.value) || 1))) : '') }),
        e('input', { placeholder: `${(run ? run.dataset_name : dataset.name) || 'Dataset'} (replan)`, value: name, style: { ...input, width: 220 }, 'aria-label': 'Name of the new dataset',
          onChange: event => setName(event.target.value) }))),
    failure ? e('div', { role: 'alert', style: { color: DANGER, fontSize: '0.75rem' } }, failure) : null,
    e('div', { style: row(8) }, spacer(),
      e(Button, { size: 'xs', disabled: busy || changes.some(c => !byId.has(c.edit)), onClick: submit }, busy ? 'Replanning…' : 'Replan and solve')))
}

function ReplanDialog({ model, run, dataset, open, onClose }) {
  return e(Dialog, { open, onOpenChange: next => { if (!next) onClose() } },
    e(DialogContent, { style: { maxWidth: 640 } },
      e(DialogHeader, null,
        e(DialogTitle, null, run ? `Replan from ${runName(run)}` : `Replan ${dataset.name}`),
        e(DialogDescription, null, 'Say what happened; the new plan starts from this one and the original stays as it is.')),
      open ? e(ReplanForm, { model, run, dataset, onDone: onClose }) : null))
}

// ---- home --------------------------------------------------------------------------

function ModelCard({ model }) {
  const { go } = useNav()
  const latest = model.latest_run
  return e(Card, { style: { minHeight: 200 } },
    e('div', { style: row(8) },
      e(Link, { to: placeOf(model.id) }, e(H2, null, model.name)),
      spacer(),
      e(Badge, { variant: model.kind === 'custom' ? 'outline' : 'muted' }, model.kind === 'custom' ? 'Yours' : 'Stock'),
      model.changed ? e(Badge, { variant: 'warn' }, 'Project changed') : null),
    // A project path has no spaces to wrap at: let it break anywhere inside the card.
    e('div', { style: { ...muted, lineHeight: 1.45, overflowWrap: 'anywhere' } }, model.summary || (model.kind === 'custom' ? `Your model, from ${model.source}` : '')),
    e('div', { style: faint }, `${count(model.constraints, 'rule')} · ${fmt(model.levels.hard)} hard · ${count(model.datasets, 'dataset')}`),
    spacer(),
    e('div', { style: { ...stack(4), borderTop: '1px solid var(--ui-stroke-tertiary)', paddingTop: 10 } },
      e('div', null, e(Verdict, { run: latest })),
      latest ? e('div', { style: faint }, `${latest.dataset_name} · ${ago(latest.finished_at || latest.created_at)}`) : null),
    e('div', { style: row(6) },
      e(Button, { size: 'xs', onClick: () => go(placeOf(model.id)) }, 'Open'),
      e(Button, { size: 'xs', variant: 'text', onClick: () => go(placeOf(model.id, 'datasets')) }, 'Datasets')))
}

function AddModelCard() {
  const { go } = useNav()
  return e(Card, { style: { minHeight: 200, borderStyle: 'dashed', justifyContent: 'center' } },
    e(H2, null, 'Add your model'),
    e('div', { style: { ...muted, lineHeight: 1.45 } }, 'Bring a solverforge-cli project: it gets datasets, profiles, solves, analysis and experiments like these.'),
    e('div', null, e(Button, { size: 'xs', variant: 'outline', onClick: () => go('/add-model') }, 'Add your model')))
}

/** Re-renders every `ms` while `on`: a clock for time used, not a fetch. */
function useTick(on, ms = 1000) {
  const [, setTick] = useState(0)
  useEffect(() => {
    if (!on) return undefined
    const timer = setInterval(() => setTick(n => n + 1), ms)
    return () => clearInterval(timer)
  }, [on, ms])
}

function RunProgress({ run }) {
  useTick(run.status === 'solving')
  const elapsed = run.started_at ? Math.max(0, Date.now() - new Date(run.started_at).getTime()) : 0
  const share = run.seconds ? Math.min(1, elapsed / (run.seconds * 1000)) : null
  return e('div', { style: stack(5) },
    e('div', { style: row(8) },
      e(Status, { status: run.status }),
      e(Link, { to: placeOf(run.model_id, 'runs', run.id) }, e('strong', null, run.dataset_name || 'Dataset')),
      spacer(),
      run.status === 'solving' && run.seconds ? e('span', { style: faint }, `${duration(elapsed)} of ${duration(run.seconds * 1000)}`) : null),
    e('div', { style: faint }, `${runName(run)} · ${profileLabel(run)}`),
    share !== null && run.status === 'solving'
      ? e('div', { role: 'progressbar', 'aria-label': 'Time used', 'aria-valuenow': Math.round(share * 100), 'aria-valuemin': 0, 'aria-valuemax': 100,
          style: { height: 5, borderRadius: 3, background: 'var(--ui-stroke-tertiary)', overflow: 'hidden' } },
          e('div', { style: { height: '100%', width: `${share * 100}%`, background: 'var(--ui-accent)' } }))
      : null,
    run.score ? e(ScoreText, { score: run.score }) : null)
}

function RunningNow() {
  const { go } = useNav()
  const query = useRead('runs', {})
  const runs = answer(query, 'runs') || []
  const live = runs.filter(r => LIVE.includes(r.status))
  const recent = runs.filter(r => !LIVE.includes(r.status)).slice(0, 6)
  return e('div', { style: stack(14) },
    e(Card, null, e(Label, null, 'Running now'),
      live.length
        ? live.map(r => e(RunProgress, { key: r.id, run: r }))
        : e('div', { style: faint }, 'Nothing is solving. A solve keeps running when you leave this page.')),
    e(Card, null, e(Label, null, 'Recent'),
      recent.length
        ? e(Table, {
            columns: [{ id: 'dot', label: '', width: 16 }, { id: 'name', label: 'Run' }, { id: 'when', label: '', num: true, width: 96 }],
            rows: recent.map(r => ({ key: r.id, id: r.id, model: r.model_id,
              dot: e(Dot, { status: r.status }),
              name: e('div', null, r.dataset_name || 'Dataset', e('div', { style: faint }, `${runName(r)} · ${profileLabel(r)}`)),
              when: e('span', { style: faint }, ago(r.finished_at || r.created_at)) })),
            onRow: r => go(placeOf(r.model, 'runs', r.id))
          })
        : e('div', { style: faint }, 'Runs you start appear here.'),
      e('div', { style: row(14) },
        e(Link, { to: '/runs', style: muted }, 'All runs →'),
        e(Link, { to: '/experiments', style: muted }, 'All experiments →'))))
}

function Home() {
  const query = useRead('models')
  const [search, setSearch] = useStored('home.search', '')
  return e(Loading, { query }, data => {
    const found = searchItems(data.models, search, [['name', 1], ['summary', 0.5], ['source', 0.4]])
    return e('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(min(100%, 420px), 1fr))', gap: 18, alignItems: 'start' } },
      e('div', { style: { ...stack(12), gridColumn: 'span 2', minWidth: 0 } },
        e('div', { style: row(10) }, e(H1, null, 'Models'), e('span', { style: muted }, 'What do you want to plan?')),
        e(ListSearch, { value: search, onChange: setSearch, shown: found.length, total: data.models.length, noun: 'model' }),
        data.models.length
          ? e('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))', gap: 14 } },
              found.map(({ item }) => e(ModelCard, { key: item.id, model: item })),
              search.trim() ? null : e(AddModelCard, null))
          : e(Empty, { title: 'No models yet', description: 'Install the engine: type /solverforge-workbench install in the chat.' }),
        search.trim() && !found.length ? e('div', { style: muted }, 'No model matches.') : null),
      e(RunningNow, null))
  })
}

// ---- your models ----------------------------------------------------------------------

/** Whether the backend runs on this computer, so a folder picked here is a path it can read. */
function useLocalBackend() {
  const [where, setWhere] = useState('checking')
  useEffect(() => {
    let live = true
    ;(async () => {
      const id = typeof host.activeConnectionId === 'function' ? host.activeConnectionId() : null
      if (!id) return 'local'
      const listed = await host.connections()
      const found = (Array.isArray(listed) ? listed : (listed && listed.connections) || []).find(c => c.id === id)
      return found && found.kind === 'local' ? 'local' : 'remote'
    })().then(value => { if (live) setWhere(value) }, () => { if (live) setWhere('local') })
    return () => { live = false }
  }, [])
  return where
}

/** A custom model build as it happens: the phase, packages compiled, cargo's last lines, the errors. */
export function BuildProgress({ build, onOpen }) {
  const share = build.total ? Math.min(1, build.compiled / build.total) : 0
  const active = build.status === 'queued' || build.status === 'building'
  const errors = (build.error && build.error.errors) || []
  return e('div', { style: stack(8) },
    e('div', { style: row(8) },
      e('span', { style: { ...row(6), display: 'inline-flex' } },
        e(Dot, { status: { queued: 'queued', building: 'solving', done: 'completed', failed: 'failed' }[build.status] }),
        { queued: 'Waiting', building: 'Building', done: 'Ready', failed: 'Failed' }[build.status] || build.status),
      e('strong', null, build.name),
      e('span', { style: muted }, build.status === 'failed' ? errorText(build.error) : build.phase),
      spacer(),
      build.total ? e('span', { style: faint }, `${fmt(build.compiled)} of ${count(build.total, 'package')}`) : null),
    active || build.status === 'done'
      ? e('div', { role: 'progressbar', 'aria-label': 'Packages compiled', 'aria-valuenow': Math.round(share * 100), 'aria-valuemin': 0, 'aria-valuemax': 100,
          style: { height: 5, borderRadius: 3, background: 'var(--ui-stroke-tertiary)', overflow: 'hidden' } },
          e('div', { style: { height: '100%', width: `${(build.status === 'done' ? 1 : share) * 100}%`, background: 'var(--ui-accent)' } }))
      : null,
    errors.length ? e('pre', { role: 'alert', style: { ...faint, color: DANGER, whiteSpace: 'pre-wrap', margin: 0 } }, errors.join('\n')) : null,
    (build.log || []).length
      ? e('pre', { 'aria-label': 'Build log', style: { ...faint, fontFamily: 'var(--font-mono, monospace)', whiteSpace: 'pre-wrap', maxHeight: 180, overflow: 'auto', margin: 0,
          background: 'var(--ui-bg-tertiary)', borderRadius: 6, padding: 8 } }, build.log.slice(-12).join('\n'))
      : null,
    build.status === 'done' && onOpen ? e('div', null, e(Button, { size: 'xs', onClick: onOpen }, `Open ${build.name}`)) : null)
}

function AddModel() {
  const { go } = useNav()
  const where = useLocalBackend()
  const [folder, setFolder] = useState(null)
  const [started, setStarted] = useState(null)
  const register = useAction('model_register', r => {
    if (r.model && !r.build) go(placeOf(r.model.id))
    else setStarted(r)
  })
  const build = useRead('model_build', { build: started && started.build ? started.build.id : '' }, { enabled: Boolean(started && started.build) })
  const current = answer(build, 'build') || (started && started.build)
  const choose = async () => {
    if (!files || typeof files.requestFolder !== 'function') {
      host.notify({ kind: 'error', message: 'This Hermes Desktop cannot pick folders. Ask Hermes to register the project instead.' })
      return
    }
    const picked = await files.requestFolder()
    if (picked && picked.ok && picked.folder) {
      setFolder(picked.folder)
      setStarted(null)
      register.mutate({ path: picked.folder.path })
    } else if (picked && !picked.ok && picked.error !== 'cancelled') {
      host.notify({ kind: 'error', message: picked.message || 'That folder cannot be used.' })
    }
  }
  return e('div', { style: { ...stack(14), maxWidth: 760 } },
    e(H1, null, 'Add your model'),
    e('div', { style: { ...muted, lineHeight: 1.5 } },
      'A solverforge-cli project becomes a model here, with everything a stock model has: datasets, profiles, solves, analysis, fill, experiments and trends. ',
      'Make one with ', e('code', null, 'solverforge new'), ' and ', e('code', null, 'solverforge generate'), ', fill in its rules, then pick its folder. ',
      'The first build takes a few minutes; later ones only rebuild what changed.'),
    where === 'remote'
      ? e(EmptyState, { title: 'Register a project on the computer that runs Hermes', description: 'This window talks to Hermes on another computer, so a folder picked here is not one it can read. Ask Hermes to register the project there.' })
      : e(Card, null,
          e('div', { style: row(8) },
            e(Button, { size: 'xs', disabled: where === 'checking' || register.isPending || Boolean(current && (current.status === 'queued' || current.status === 'building')), onClick: choose },
              folder ? 'Choose another folder' : 'Choose the project folder'),
            folder ? e('span', { style: faint }, folder.path) : null),
          register.isPending ? e('div', { style: muted }, 'Reading the project…') : null,
          started && (started.warnings || []).length
            ? e('div', { style: stack(2) }, e(Label, null, 'Before you solve'),
                e('ul', { style: { margin: 0, paddingLeft: 18, ...muted } }, started.warnings.map((w, i) => e('li', { key: i }, errorText({ error: typeof w === 'string' ? w : w.message })))))
            : null,
          current ? e(BuildProgress, { build: current, onOpen: () => go(placeOf(current.model_id)) }) : null))
}

/** A custom model's own project: where it lives, whether it changed, rebuild and remove. */
function ProjectCard({ model }) {
  const { go } = useNav()
  const builds = useRead('model_build', { model: model.id })
  const latest = (answer(builds, 'builds') || [])[0]
  const active = latest && (latest.status === 'queued' || latest.status === 'building')
  const rebuild = useAction('model_register', r => { if (r.model && !r.build) host.notify({ kind: 'success', message: `${model.name} is up to date.` }) })
  return e(Card, null, e(Label, null, 'Your project'),
    e('div', { style: { ...faint, overflowWrap: 'anywhere' } }, model.source),
    model.changed ? e('div', null, e(Badge, { variant: 'warn' }, 'Project changed since this build')) : null,
    latest && (active || latest.status === 'failed') ? e(BuildProgress, { build: latest }) : null,
    e('div', { style: row(6) },
      e(Button, { size: 'xs', variant: model.changed ? 'default' : 'outline', disabled: active || rebuild.isPending,
        onClick: () => rebuild.mutate({ path: model.source, model: model.id }) }, active ? 'Building…' : 'Rebuild'),
      e(DeleteButton, { action: 'model_remove', args: { model: model.id }, what: 'this model', label: 'Remove', onDone: () => go('/home') })))
}

// ---- a model's hub -----------------------------------------------------------------

const MODEL_TABS = [
  ['overview', 'Overview'], ['datasets', 'Datasets'], ['runs', 'Runs'], ['profiles', 'Profiles'], ['experiments', 'Experiments'], ['trends', 'Trends']
]

function ModelHub({ place }) {
  const { go } = useNav()
  const query = useRead('model', { model: place.model })
  const datasets = useRead('datasets', { model: place.model })
  const runs = useRead('runs', { model: place.model })
  const experiments = useRead('experiments', { model: place.model })
  const tally = (q, key) => (answer(q, key) ? answer(q, key).length : null)
  return e(Loading, { query }, ({ model }) => {
    const counts = { datasets: tally(datasets, 'datasets'), runs: tally(runs, 'runs'), profiles: model.profiles.length, experiments: tally(experiments, 'experiments') }
    const body = {
      overview: () => e(Overview, { model }),
      datasets: () => (place.id ? e(DatasetPage, { key: place.id, model, id: place.id, tab: place.extra }) : e(DatasetsTab, { model })),
      runs: () => (place.id === 'compare' ? e(ComparePage, { model, ids: place.extra || '' }) : place.id ? e(RunPage, { key: place.id, model, id: place.id }) : e(RunsTab, { model })),
      profiles: () => e(ProfilesTab, { model, picked: place.id }),
      experiments: () => (place.id === 'new' ? e(ExperimentNew, { model, datasets: place.extra ? place.extra.split(',') : [], profiles: place.also ? place.also.split(',') : [] })
        : place.id ? e(ExperimentPage, { key: place.id, model, id: place.id }) : e(ExperimentsTab, { model })),
      trends: () => e(TrendsTab, { model })
    }[place.tab] || (() => e(Overview, { model }))
    return e('div', { style: stack(14) },
      e('div', { style: row(10) },
        e(H1, null, model.name),
        e('span', { style: muted }, `${model.kind === 'custom' ? 'Your model' : 'Stock model'} · ${count(model.constraints.length, 'rule')}`),
        model.changed ? e(Badge, { variant: 'warn' }, 'Project changed') : null),
      e(TabBar, { value: place.tab, onChange: tab => go(placeOf(model.id, tab)),
        tabs: MODEL_TABS.map(([id, label]) => ({ id, label, count: id === 'overview' || id === 'trends' ? null : counts[id] })) }),
      body())
  })
}

function LevelCounts({ constraints }) {
  const counts = { hard: 0, medium: 0, soft: 0 }
  for (const c of constraints) counts[c.level] = (counts[c.level] || 0) + 1
  return e('div', { style: row(10) }, ['hard', 'medium', 'soft'].filter(l => counts[l]).map(l =>
    e('span', { key: l, style: { color: levelColor(l), fontSize: '0.75rem', fontWeight: 600 } }, `${fmt(counts[l])} ${l}`)))
}

function Overview({ model }) {
  const { go } = useNav()
  const datasets = useRead('datasets', { model: model.id })
  const runs = useRead('runs', { model: model.id })
  const create = useAction('dataset_create', r => r.dataset && go(placeOf(model.id, 'datasets', r.dataset.id)))
  const latest = (answer(runs, 'runs') || []).find(r => r.kind === 'solve' && FINISHED.includes(r.status))
  const groups = []
  for (const c of model.constraints) {
    const g = c.group || 'Rules'
    const found = groups.find(x => x[0] === g)
    if (found) found[1] += 1
    else groups.push([g, 1])
  }
  return e('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(min(100%, 380px), 1fr))', gap: 16, alignItems: 'start' } },
    e('div', { style: { ...stack(16), gridColumn: 'span 2', minWidth: 0 } },
      model.summary ? e('div', { style: { ...muted, fontSize: '0.85rem', lineHeight: 1.5 } }, model.summary) : null,
      e(NewDataset, { model, create }),
      e(Card, null,
        e('div', { style: row(8) }, e(Label, null, 'Your datasets'), spacer(),
          e(Link, { to: placeOf(model.id, 'datasets'), style: muted }, 'All datasets →')),
        e(Loading, { query: datasets }, data => e(DatasetTable, { model, datasets: data.datasets.slice(0, 5).map(item => ({ item, hits: [] })),
          empty: e('div', { style: faint }, 'None yet. Start from a demo above.') })))),
    e('div', { style: stack(16) },
      e(Card, null, e(Label, null, 'Latest result'),
        latest
          ? e('div', { style: stack(8) },
              e('div', null, e(Verdict, { run: latest })),
              e('div', null, e(ScoreText, { score: latest.score })),
              e('div', { style: faint }, `${latest.dataset_name} · ${profileLabel(latest)} · ${ago(latest.finished_at || latest.created_at)}`),
              e('div', null, e(Button, { size: 'xs', variant: 'outline', onClick: () => go(placeOf(model.id, 'runs', latest.id)) }, 'Open run')))
          : e('div', { style: faint }, 'Nothing solved yet.')),
      e(Card, null, e(Label, null, 'How it plans'),
        e(LevelCounts, { constraints: model.constraints }),
        e('div', { style: { ...faint, lineHeight: 1.6 } }, groups.map(([g, n]) => `${g} ${n}`).join(' · ')),
        e('div', null, e(Button, { size: 'xs', variant: 'outline', onClick: () => go(placeOf(model.id, 'profiles')) }, 'Rules and weights'))),
      model.kind === 'custom' ? e(ProjectCard, { model }) : null))
}

// ---- new datasets -------------------------------------------------------------------

/** What a check found: whether the model reads the data, its size, its issues and its score as it stands. */
export function CheckResult({ checked }) {
  const errors = (checked.issues || []).filter(i => i.severity === 'error')
  const notes = (checked.issues || []).filter(i => i.severity !== 'error')
  const list = (items, color) => e('ul', { style: { margin: '2px 0', paddingLeft: 18, ...muted } },
    items.slice(0, 12).map((i, n) => e('li', { key: n, style: color ? { color } : undefined }, i.message)),
    items.length > 12 ? e('li', { style: faint }, `… ${count(items.length - 12, 'more issue')}`) : null)
  if (!checked.valid) {
    return e('div', { style: stack(6), role: 'alert' },
      e('div', { style: row(8) }, e(Badge, { variant: 'destructive' }, 'Cannot be read'), e('span', { style: muted }, sizeText(checked.size))),
      list(errors.length ? errors : checked.issues || [], DANGER))
  }
  return e('div', { style: stack(8) },
    e('div', { style: row(8) },
      e(Badge, { variant: 'success' }, 'Reads'), e('span', { style: muted }, sizeText(checked.size))),
    e('div', { style: row(8) },
      e(Verdict, { run: { score: checked.score } }), e(ScoreText, { score: checked.score }),
      e('span', { style: faint }, `as it stands, scored by ${checked.profile || 'Standard'}`)),
    checked.costing && checked.costing.length
      ? e(Table, { columns: [{ id: 'name', label: 'What costs score as it stands' }, { id: 'level', label: 'Level', width: 80 }, { id: 'score', label: 'Score', num: true, width: 220 }],
          rows: checked.costing.slice(0, 8).map(c => ({ key: c.name, name: c.name, level: e('span', { style: { color: levelColor(c.level) } }, c.level), score: e(ScoreText, { score: c.score }) })) })
      : null,
    notes.length ? e('div', { style: stack(2) }, e(Label, null, 'Notes'), list(notes)) : null)
}

export function ImportDataset({ model }) {
  const { go } = useNav()
  const [file, setFile] = useState(null)
  const [checked, setChecked] = useState(null)
  const [name, setName] = useState('')
  const [tags, setTags] = useState('')
  const [busy, setBusy] = useState(false)
  const check = useAction('dataset_check', setChecked)
  const pick = picked => {
    setFile(picked)
    setChecked(null)
    setName(picked.name)
    if (!picked.error) check.mutate({ model: model.id, plan: picked.value })
  }
  const create = async solve => {
    setBusy(true)
    try {
      const made = await rest('/actions/dataset_create', { method: 'POST', body: { model: model.id, plan: file.value, name: name.trim() || file.name, tags: parseTags(tags) } })
      if (!made || !made.ok) { host.notify({ kind: 'error', message: errorText(made) }); return }
      if (solve) {
        const run = await rest('/actions/solve', { method: 'POST', body: { dataset: made.dataset.id } })
        if (run && run.ok) { go(placeOf(model.id, 'runs', run.run.id)); return }
        host.notify({ kind: 'error', message: errorText(run) })
      }
      go(placeOf(model.id, 'datasets', made.dataset.id))
    } catch (error) {
      host.notify({ kind: 'error', message: errorText({ error: error && error.message }) })
    } finally {
      queryClient.invalidateQueries({ queryKey: [ID] })
      setBusy(false)
    }
  }
  return e('div', { style: stack(10) },
    e('div', { style: row(8) },
      e(JsonFileButton, { label: file ? 'Choose another file' : 'Choose a JSON file', onFile: pick }),
      e('span', { style: faint }, file ? file.name : `A problem in ${model.name}’s format. A demo, exported, shows the format.`)),
    file && file.error ? e('div', { role: 'alert', style: muted }, file.error) : null,
    check.isPending ? e(Skeleton, { style: { height: 60, width: '100%' } }) : null,
    checked ? e(CheckResult, { checked }) : null,
    checked && checked.valid
      ? e('div', { style: row(8) },
          e('input', { value: name, 'aria-label': 'Dataset name', placeholder: 'Name', style: { ...input, width: 220 }, onChange: event => setName(event.target.value) }),
          e('input', { value: tags, 'aria-label': 'Tags', placeholder: 'tags, comma separated', style: { ...input, width: 200 }, onChange: event => setTags(event.target.value) }),
          e(Button, { size: 'xs', variant: 'outline', disabled: busy, onClick: () => create(false) }, 'Create'),
          e(Button, { size: 'xs', disabled: busy, onClick: () => create(true) }, 'Create and solve'))
      : null)
}

export function CopyDataset({ model }) {
  const { go } = useNav()
  const datasets = useRead('datasets', { model: model.id })
  const [source, setSource] = useState('')
  const [name, setName] = useState('')
  const copy = useAction('dataset_patch', r => r.dataset && go(placeOf(model.id, 'datasets', r.dataset.id)))
  const all = answer(datasets, 'datasets') || []
  if (!all.length) return e('div', { style: faint }, 'There is no dataset to copy yet.')
  return e('div', { style: row(8) },
    e('select', { value: source, style: input, 'aria-label': 'Dataset to copy', onChange: event => setSource(event.target.value) },
      e('option', { value: '' }, 'Pick a dataset'),
      all.map(d => e('option', { key: d.id, value: d.id }, `${d.name} · ${sizeText(d.size)}`))),
    e('input', { value: name, 'aria-label': 'Name of the copy', placeholder: 'Name of the copy', style: { ...input, width: 220 }, onChange: event => setName(event.target.value) }),
    e(Button, { size: 'xs', disabled: !source || copy.isPending, onClick: () => copy.mutate({ dataset: source, operations: [], ...(name.trim() ? { name: name.trim() } : {}) }) }, 'Copy'))
}

/** One place, three sources: a demo, a file checked before it is saved, or a copy. */
function NewDataset({ model, create }) {
  const [source, setSource] = useStored(`${model.id}.new.source`, 'demo')
  const demos = model.demo_info || []
  return e(Card, null,
    e('div', { style: row(8) }, e(Label, null, 'New dataset')),
    e(TabBar, { value: source, onChange: setSource, tabs: [{ id: 'demo', label: 'From a demo', count: demos.length }, { id: 'import', label: 'Import a file' }, { id: 'copy', label: 'Copy a dataset' }] }),
    source === 'import' ? e(ImportDataset, { model })
      : source === 'copy' ? e(CopyDataset, { model })
      : demos.length
        ? e('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: 12 } },
            demos.map(d => e('div', { key: d.id, style: { ...card, ...stack(8) } },
              e('strong', null, d.name),
              e('div', { style: { ...faint, lineHeight: 1.5 } }, d.summary || 'A sample dataset that comes with the model.'),
              e('div', null, e(Button, { size: 'xs', disabled: create.isPending, onClick: () => create.mutate({ model: model.id, demo: d.id, name: d.name }) },
                create.isPending ? 'Creating…' : 'Use this demo')))))
        : e('div', { style: faint }, 'This model has no demo data. Import a file instead.'))
}

// ---- datasets ----------------------------------------------------------------------

function DatasetTable({ model, datasets, empty }) {
  const { go } = useNav()
  return e(Table, {
    empty,
    columns: [{ id: 'name', label: 'Dataset' }, { id: 'from', label: 'Made from' }, { id: 'size', label: 'Size' }, { id: 'latest', label: 'Latest' }, { id: 'when', label: 'Made', num: true, width: 110 }],
    rows: datasets.map(({ item: d, hits }) => ({
      key: d.id, id: d.id,
      name: e('div', null, e('strong', null, e(Highlight, { text: d.name, hits })),
        (d.tags || []).length ? e('div', { style: { ...row(4), marginTop: 3 } }, d.tags.map(t => e(Badge, { key: t, variant: 'outline' }, t))) : null),
      from: e('span', { style: muted }, madeFrom(d)),
      size: e('span', { style: muted }, sizeText(d.size)),
      latest: d.latest_run ? e('div', { style: row(8) }, e(Dot, { status: d.latest_run.status }), e(ScoreText, { score: d.latest_run.score })) : e('span', { style: faint }, 'Not solved'),
      when: e('span', { style: faint }, ago(d.created_at))
    })),
    onRow: r => go(placeOf(model.id, 'datasets', r.id))
  })
}

const ORIGINS = [['', 'any'], ['demo', 'Demo'], ['upload', 'Imported'], ['run', 'Replan'], ['fill', 'Fill'], ['patch', 'Edited']]

function DatasetsTab({ model }) {
  const { go } = useNav()
  const query = useRead('datasets', { model: model.id })
  const [search, setSearch] = useStored(`${model.id}.datasets.search`, '')
  const [origin, setOrigin] = useStored(`${model.id}.datasets.origin`, '')
  const [sort, setSort] = useStored(`${model.id}.datasets.sort`, 'new')
  useKeys({ n: () => go(placeOf(model.id)) })
  return e(Loading, { query }, data => {
    const pool = data.datasets.filter(d => !origin || d.origin === origin)
    let found = searchItems(pool, search, [['name', 1], [d => (d.tags || []).join(' '), 0.7], [d => madeFrom(d), 0.5]])
    if (!search.trim() && sort === 'name') found = [...found].sort((a, b) => a.item.name.localeCompare(b.item.name))
    return e('div', { style: stack(10) },
      e(ListSearch, { value: search, onChange: setSearch, shown: found.length, total: data.datasets.length, noun: 'dataset', placeholder: 'Search datasets: name, tag or where it came from' },
        e(Pick, { label: 'Made from', value: origin, onChange: setOrigin, options: ORIGINS }),
        e(Pick, { label: 'Sort', value: sort, onChange: setSort, options: [['new', 'newest first'], ['name', 'by name']] }),
        e(Button, { size: 'xs', onClick: () => go(placeOf(model.id)) }, 'New dataset')),
      e(DatasetTable, { model, datasets: found,
        empty: data.datasets.length
          ? e('div', { style: muted }, 'No dataset matches.')
          : e(Empty, { title: 'No datasets yet', description: 'A dataset is one planning problem. Start from a demo on the overview.',
              action: e(Button, { size: 'xs', onClick: () => go(placeOf(model.id)) }, 'Start') }) }))
  })
}

function ProfilePicker({ model, value, onChange }) {
  const fallback = model.profiles.find(p => p.id === model.default_profile) || { name: 'Standard' }
  return e('select', { value, style: input, 'aria-label': 'Profile', onChange: event => onChange(event.target.value) },
    e('option', { value: '' }, `${fallback.name} (default)`),
    model.profiles.filter(p => p.id !== model.default_profile).map(p => e('option', { key: p.id, value: p.id }, p.name)))
}

/** "a, b ,c" -> ['a', 'b', 'c']. */
export const parseTags = text => [...new Set(String(text || '').split(',').map(t => t.trim()).filter(Boolean))]

function DatasetHeader({ model, dataset }) {
  const { go } = useNav()
  const [editing, setEditing] = useState(null)
  const update = useAction('dataset_update', () => setEditing(null))
  const duplicate = useAction('dataset_patch', r => r.dataset && go(placeOf(model.id, 'datasets', r.dataset.id)))
  if (editing) {
    return e('form', { style: row(8), onSubmit: event => {
      event.preventDefault()
      update.mutate({ dataset: dataset.id, name: editing.name.trim() || dataset.name, tags: parseTags(editing.tags) })
    } },
      e('input', { value: editing.name, autoFocus: true, 'aria-label': 'Dataset name', style: { ...input, width: 240 },
        onChange: event => setEditing({ ...editing, name: event.target.value }), onKeyDown: event => { if (event.key === 'Escape') setEditing(null) } }),
      e('input', { value: editing.tags, placeholder: 'tags, comma separated', 'aria-label': 'Tags', style: { ...input, width: 220 },
        onChange: event => setEditing({ ...editing, tags: event.target.value }) }),
      e(Button, { size: 'xs', type: 'submit', disabled: update.isPending }, 'Save'),
      e(Button, { size: 'xs', variant: 'text', onClick: () => setEditing(null) }, 'Cancel'))
  }
  return e('div', { style: row(8) },
    e(H2, null, dataset.name), e(CopyId, { id: dataset.id, what: 'dataset' }),
    (dataset.tags || []).map(t => e(Badge, { key: t, variant: 'outline' }, t)),
    spacer(),
    e(Button, { size: 'xs', variant: 'text', onClick: () => setEditing({ name: dataset.name, tags: (dataset.tags || []).join(', ') }) }, 'Rename'),
    e(Button, { size: 'xs', variant: 'text', disabled: duplicate.isPending, onClick: () => duplicate.mutate({ dataset: dataset.id, operations: [] }) }, 'Duplicate'),
    e(ExportButton, { action: 'export', args: { dataset: dataset.id }, name: dataset.name }),
    e(DeleteButton, { action: 'dataset_delete', args: { dataset: dataset.id }, what: 'this dataset', onDone: () => go(placeOf(model.id, 'datasets')) }))
}

function DatasetPage({ model, id, tab: wanted }) {
  const { go } = useNav()
  const query = useRead('dataset', { dataset: id })
  const [profile, setProfile] = useState('')
  const [seconds, setSeconds] = useState('')
  const [freeze, setFreeze] = useState('')
  const [selected, setSelected] = useState([])
  const [shown, setShown] = useState(false)
  const solve = useAction('solve', r => r.run && go(placeOf(model.id, 'runs', r.run.id)))
  const evaluate = useAction('evaluate', r => r.run && go(placeOf(model.id, 'runs', r.run.id)))
  const [replanning, setReplanning] = useState(false)
  const solveSettings = { ...(seconds ? { seconds: Number(seconds) } : {}), ...(freeze ? { freeze_before: freeze.slice(0, 16) } : {}) }
  const body = { dataset: id, ...(profile ? { profile } : {}), ...(Object.keys(solveSettings).length ? { overrides: { solve: solveSettings } } : {}) }
  useKeys({ s: () => solve.mutate(body), e: () => go(placeOf(model.id, 'experiments', 'new', id)), r: () => setReplanning(true) })
  return e(Loading, { query }, ({ dataset }) => e('div', { style: stack(14) },
    e(DatasetHeader, { model, dataset }),
    e('div', { style: { ...row(8), ...muted } },
      e('span', null, sizeText(dataset.size)),
      e('span', { style: faint }, '·'),
      dataset.parent_id
        ? e('span', null, `${madeFrom({ ...dataset, parent_name: '' }).replace(/ another dataset$/, '')} `,
            e(Link, { to: placeOf(model.id, 'datasets', dataset.parent_id), style: { textDecoration: 'underline' } }, dataset.parent_name || 'its parent'))
        : e('span', null, madeFrom(dataset)),
      e('span', { style: faint }, `· ${ago(dataset.created_at)}`),
      dataset.travel ? e('span', { style: faint }, `· ${TRAVEL[dataset.travel]}`) : null),
    e(LineageStrip, { model: model.id, lineage: dataset.lineage, here: dataset.name }),
    dataset.patch && dataset.patch.some(p => p.op === 'edit' || p.op === 'fill')
      ? e('ul', { 'aria-label': 'What was changed', style: { margin: 0, paddingLeft: 18, ...muted } },
          dataset.patch.filter(p => p.op === 'edit' || p.op === 'fill').map((p, n) =>
            e('li', { key: n }, p.op === 'edit' ? p.summary : `Placed ${p.item} at ${p.value}.`)))
      : null,
    e(Card, null,
      e('div', { style: row(8) },
        e(ProfilePicker, { model, value: profile, onChange: setProfile }),
        e('input', { type: 'number', min: 1, placeholder: 'seconds', value: seconds, style: { ...input, width: 84 }, 'aria-label': 'Time limit in seconds',
          onChange: event => setSeconds(event.target.value ? String(Math.max(1, Math.trunc(Number(event.target.value) || 1))) : '') }),
        e('label', { style: { ...row(4), ...faint } }, 'Keep everything before',
          e('input', { type: 'datetime-local', value: freeze, style: input, 'aria-label': 'Keep everything that starts before', onChange: event => setFreeze(event.target.value) })),
        spacer(),
        e(Button, { size: 'xs', variant: 'outline', onClick: () => setReplanning(true) }, 'Change and solve…'),
        e(Button, { size: 'xs', variant: 'outline', onClick: () => go(placeOf(model.id, 'experiments', 'new', id)) }, 'Compare profiles…'),
        e(Button, { size: 'xs', variant: 'outline', disabled: evaluate.isPending, onClick: () => evaluate.mutate(body) }, 'Check as is'),
        e(Button, { size: 'xs', disabled: solve.isPending, onClick: () => solve.mutate(body) }, solve.isPending ? 'Starting…' : 'Solve')),
      freeze ? e('div', { style: faint }, `Everything that starts before ${freeze.replace('T', ' ')} keeps its place.`) : null),
    (() => {
      const tab = ['plan', 'data', 'runs'].includes(wanted) ? wanted : dataset.runs.length ? 'runs' : 'plan'
      return e('div', { style: stack(10) },
        e(TabBar, { tabs: [{ id: 'plan', label: 'Plan' }, { id: 'data', label: 'Data' }, { id: 'runs', label: 'Runs', count: dataset.runs.length }],
          value: tab, onChange: next => go(placeOf(model.id, 'datasets', dataset.id, next)) }),
        tab === 'plan'
          ? e('div', { style: stack(10) },
              model.view
                ? e('div', { style: stack(8) },
                    e('div', null, e(Button, { size: 'xs', variant: 'text', onClick: () => setShown(!shown) }, `${shown ? 'Hide' : 'Show'} the input ${viewName(model.view).toLowerCase()}`)),
                    shown ? e(PlanView, { dataset: dataset.id, kind: model.view }) : null)
                : null,
              e(FillPanel, { dataset: dataset.id, profile, model: model.id }))
          : tab === 'data'
            ? e(DataEditor, { model, dataset })
            : e(Card, null,
                e('div', { style: row(8) }, e(Label, null, 'Runs'), spacer(),
                  selected.length >= 2 ? e(Button, { size: 'xs', variant: 'secondary', onClick: () => go(placeOf(model.id, 'runs', 'compare', selected.join(','))) }, `Compare ${selected.length}`) : null),
                e(RunTable, { model, runs: dataset.runs.map(item => ({ item, hits: [] })), selected, showDataset: false,
                  onSelect: r => setSelected(selected.includes(r) ? selected.filter(x => x !== r) : [...selected, r].slice(-4)),
                  empty: e('div', { style: faint }, 'Not solved yet. Solve it above.') })))
    })(),
    e(ReplanDialog, { model, dataset, open: replanning, onClose: () => setReplanning(false) })))
}

// ---- the data editor ----------------------------------------------------------------

// The rows at a table's path ("visits", or each vehicle's shifts as vehicles/STAR/shifts with
// STAR the wildcard), each with the JSON Patch path that addresses it by id and its parent row.
export function tableRows(plan, path) {
  let level = [{ value: plan, at: '', parent: null }]
  for (const part of path.split('/')) {
    if (part === '*') {
      level = level.flatMap(({ value, at }) => (Array.isArray(value) ? value : []).map(row => ({ value: row, at: `${at}/[id=${row.id}]`, parent: row })))
    } else {
      level = level.map(({ value, at, parent }) => ({ value: value && value[part], at: `${at}/${part}`, parent }))
    }
  }
  return level.flatMap(({ value, at, parent }) => (Array.isArray(value) ? value : []).map(row => ({ row, at: `${at}/[id=${row.id}]`, parent })))
}

const kindOf = value => (typeof value === 'boolean' ? 'boolean' : typeof value === 'number' ? (Number.isInteger(value) ? 'integer' : 'number')
  : typeof value === 'string' ? (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(value) ? 'date_time' : 'text') : 'json')

/** Tables for a model that describes none (a custom model): every top-level list of records
 *  with ids, its columns read from the records themselves. */
export function inferTables(plan) {
  return Object.entries(plan || {})
    .filter(([, v]) => Array.isArray(v) && v.length && v.every(r => r && typeof r === 'object' && !Array.isArray(r) && 'id' in r))
    .map(([key, rows]) => {
      const names = [...new Set(rows.flatMap(r => Object.keys(r)))].filter(n => n !== 'id')
      return {
        id: key, label: key[0].toUpperCase() + key.slice(1), one: key.replace(/s$/, ''), path: key, title: names.includes('name') ? 'name' : '',
        columns: names.map(name => {
          const seen = rows.map(r => r[name]).filter(v => v !== undefined && v !== null)
          const kinds = [...new Set(seen.map(kindOf))]
          return { name, label: name, kind: kinds.length === 1 ? kinds[0] : 'json', optional: seen.length < rows.length }
        })
      }
    })
}

/** A column as the editor of its value: references become choices among the rows they name. */
function asParam(column, choices) {
  const options = column.reference
    ? [...(column.optional ? [{ value: '', label: 'none' }] : []), ...(choices[column.reference] || [])]
    : column.options
  return { ...column, kind: column.reference ? 'text' : column.kind, options, fields: (column.fields || []).map(f => asParam(f, choices)) }
}

/** A value in a few words for the grid. */
export function cellText(column, value) {
  if (value === undefined || value === null || value === '') return '—'
  if (column.kind === 'location' && Array.isArray(value)) return value.map(v => Number(v).toFixed(4)).join(', ')
  if (column.kind === 'date_time') return String(value).slice(0, 16).replace('T', ' ')
  if (column.kind === 'list' && Array.isArray(value)) {
    const item = (column.fields || [])[0] || {}
    if (!value.length) return '—'
    if (item.kind === 'object') {
      return value.map(v => (item.fields || []).filter(f => v && v[f.name] !== undefined && v[f.name] !== null)
        .map(f => cellText(f, v[f.name])).join(' · ')).join('; ')
    }
    return value.map(v => cellText(item, v)).join(', ')
  }
  if (column.kind === 'object' && value && typeof value === 'object') {
    return (column.fields || []).filter(f => value[f.name] !== undefined).map(f => `${f.label.toLowerCase()} ${cellText(f, value[f.name])}`).join(' · ')
  }
  if (typeof value === 'object') return JSON.stringify(value)
  return paramText(column, value)
}

/** Staged changes as id-addressed JSON Patch operations against the plan as stored. */
export function stagedOperations(staged) {
  return Object.values(staged)
    .filter(c => c.value !== undefined || c.had)
    .map(c => (c.value === undefined ? { op: 'remove', path: c.path } : { op: c.had ? 'replace' : 'add', path: c.path, value: c.value }))
}

export function DataEditor({ model, dataset }) {
  const { go } = useNav()
  const query = useRead('dataset', { dataset: dataset.id, include_plan: true })
  const [staged, setStaged] = useState({})
  const [tab, setTab] = useStored(`${model.id}.data.tab`, '')
  const [search, setSearch] = useState('')
  const [open, setOpen] = useState(null)
  const [name, setName] = useState(`${dataset.name}, edited`)
  const save = useAction('dataset_patch', r => {
    setStaged({})
    if (r.dataset && r.dataset.id !== dataset.id) go(placeOf(model.id, 'datasets', r.dataset.id, 'data'))
  })
  return e(Loading, { query }, ({ dataset: full }) => {
    const plan = full.plan || {}
    const tables = (model.tables && model.tables.length ? model.tables : inferTables(plan))
    if (!tables.length) return e('div', { style: faint }, 'This model’s data has no lists of records to edit here. Export it, change the JSON and import it as a new dataset.')
    const current = tables.find(t => t.id === tab) || tables[0]
    const rowsOf = t => tableRows(plan, t.path)
    const titleOf = (t, row) => (t.title && row[t.title]) || row.id
    const choices = Object.fromEntries(tables.map(t => [t.id, rowsOf(t).map(({ row }) => ({ value: String(row.id), label: titleOf(t, row) === row.id ? String(row.id) : `${titleOf(t, row)} (${row.id})` }))]))
    const valueOf = (at, row, column) => {
      const change = staged[`${at}/${column.name}`]
      return change ? change.value : row[column.name]
    }
    const stage = (t, at, row, column, value) => {
      const path = `${at}/${column.name}`
      const had = row[column.name] !== undefined
      const same = JSON.stringify(value) === JSON.stringify(row[column.name])
      const next = { ...staged }
      if (same) delete next[path]
      else next[path] = { path, had, value, words: `${t.one[0].toUpperCase()}${t.one.slice(1)} ${titleOf(t, row)}: ${column.label.toLowerCase()} ${cellText(column, value)}` }
      setStaged(next)
    }
    const all = rowsOf(current)
    const found = searchItems(all, search, [[({ row }) => titleOf(current, row), 1], [({ row }) => String(row.id), 0.9],
      ...current.columns.map(c => [({ row, at }) => cellText(c, valueOf(at, row, c)), 0.5])])
    const shown = current.columns.slice(0, 5)
    const changes = Object.values(staged)
    const operations = stagedOperations(staged)
    const hasRuns = (dataset.runs || []).length > 0
    return e('div', { style: stack(10) },
      e(TabBar, { tabs: tables.map(t => ({ id: t.id, label: t.label, count: rowsOf(t).length })), value: current.id, onChange: id => { setTab(id); setSearch(''); setOpen(null) } }),
      e(ListSearch, { value: search, onChange: setSearch, shown: found.length, total: all.length, noun: current.one, placeholder: `Search ${current.label.toLowerCase()}` }),
      e(Table, {
        empty: e('div', { style: faint }, `No ${current.label.toLowerCase()}.`),
        columns: [{ id: '_title', label: current.title ? 'Name' : 'Id' }, ...(current.parent ? [{ id: '_parent', label: 'Of' }] : []), ...shown.map(c => ({ id: c.name, label: c.label }))],
        rows: found.slice(0, 200).map(({ item: { row, at, parent }, hits }) => ({
          key: at, at,
          _title: e('span', { style: { fontWeight: open === at ? 600 : 400 } }, e(Highlight, { text: String(titleOf(current, row)), hits }),
            Object.keys(staged).some(p => p.startsWith(`${at}/`)) ? e('span', { style: { color: 'var(--ui-accent)' } }, ' •') : null),
          _parent: parent ? e('span', { style: faint }, (() => { const t = tables.find(x => x.id === current.parent); return t ? titleOf(t, parent) : parent.id })()) : null,
          ...Object.fromEntries(shown.map(c => [c.name, e('span', { style: muted }, cellText(c, valueOf(at, row, c)))]))
        })),
        onRow: r => setOpen(open === r.at ? null : r.at)
      }),
      found.length > 200 ? e('div', { style: faint }, `Showing the first 200 of ${fmt(found.length)}; search to narrow them.`) : null,
      (() => {
        const hit = all.find(x => x.at === open)
        if (!hit) return null
        return e(Card, null,
          e('div', { style: row(8) }, e(Label, null, `${current.one[0].toUpperCase()}${current.one.slice(1)} ${titleOf(current, hit.row)}`), spacer(),
            e(Button, { size: 'xs', variant: 'text', onClick: () => setOpen(null) }, 'Close')),
          e('div', { style: { display: 'grid', gridTemplateColumns: 'minmax(120px, max-content) 1fr', gap: '8px 14px', alignItems: 'start' } },
            current.columns.flatMap(c => [
              e('span', { key: `${c.name}-l`, style: muted }, c.label),
              e(ParamCell, { key: `${c.name}-v`, param: asParam(c, choices), value: valueOf(hit.at, hit.row, c), changed: Boolean(staged[`${hit.at}/${c.name}`]),
                onChange: v => stage(current, hit.at, hit.row, c, c.reference && v === '' ? undefined : v) })])))
      })(),
      changes.length
        ? e(Card, null,
            e(Label, null, `${count(changes.length, 'change')} not saved`),
            e('ul', { 'aria-label': 'Changes not saved', style: { margin: 0, paddingLeft: 18, ...muted } }, changes.map(c => e('li', { key: c.path }, c.words))),
            e('div', { style: row(8) },
              e('input', { value: name, 'aria-label': 'Name of the new dataset', style: { ...input, width: 220 }, onChange: event => setName(event.target.value) }),
              e(Button, { size: 'xs', disabled: save.isPending, onClick: () => save.mutate({ dataset: dataset.id, operations, name }) }, 'Save as a new dataset'),
              e(Button, { size: 'xs', variant: 'outline', disabled: save.isPending || hasRuns, title: hasRuns ? 'Runs used this dataset as it is; save the change as a new one.' : undefined,
                onClick: () => save.mutate({ dataset: dataset.id, operations, in_place: true }) }, 'Save here'),
              e(Button, { size: 'xs', variant: 'text', onClick: () => setStaged({}) }, 'Discard')),
            e('div', { style: faint }, 'The model reads the changed data before it is saved; nothing is saved if it cannot.'))
        : e('div', { style: faint }, 'Click a row to change it. Changes stay here until you save them.'))
  })
}

// ---- runs --------------------------------------------------------------------------

function RunTable({ model, runs, selected = [], onSelect, empty, showDataset = true, showModel = false }) {
  const { go } = useNav()
  return e(Table, {
    empty,
    columns: [
      ...(onSelect ? [{ id: 'pick', label: '', width: 28 }] : []),
      { id: 'name', label: 'Run' },
      ...(showModel ? [{ id: 'model', label: 'Model' }] : []),
      ...(showDataset ? [{ id: 'dataset', label: 'Dataset' }] : []),
      { id: 'profile', label: 'Profile' }, { id: 'status', label: 'Status', width: 120 }, { id: 'score', label: 'Score' },
      { id: 'time', label: 'Time', num: true, width: 70 }
    ],
    rows: runs.map(({ item: r, hits }) => ({
      key: r.id, id: r.id, modelId: r.model_id,
      pick: onSelect ? e('input', { type: 'checkbox', checked: selected.includes(r.id), 'aria-label': `Compare ${runName(r)}`,
        onClick: event => event.stopPropagation(), onChange: () => onSelect(r.id) }) : null,
      name: e('div', null, e(Highlight, { text: runName(r), hits }), e('div', { style: faint }, ago(r.created_at))),
      model: e('span', { style: muted }, r.model_name || r.model_id),
      dataset: e('span', { style: muted }, r.dataset_name || '—'),
      profile: e('span', { style: muted }, profileLabel(r)),
      status: e(Status, { status: r.status }),
      score: e(ScoreText, { score: r.score }),
      time: e('span', { style: faint }, r.elapsed_ms != null && FINISHED.includes(r.status) ? duration(r.elapsed_ms) : '—')
    })),
    onRow: r => go(placeOf(model ? model.id : r.modelId, 'runs', r.id))
  })
}

const STATUSES = [['', 'any'], ...Object.entries(STATUS).map(([k, [word]]) => [k, word])]
const RUN_FIELDS = [[r => runName(r), 1], ['dataset_name', 0.8], ['profile_name', 0.6], [r => (STATUS[r.status] || [r.status])[0], 0.5]]
const pickRuns = (selected, run) => (selected.includes(run) ? selected.filter(x => x !== run) : [...selected, run].slice(-4))

function RunsTab({ model }) {
  const { go } = useNav()
  const query = useRead('runs', { model: model.id })
  const [search, setSearch] = useStored(`${model.id}.runs.search`, '')
  const [status, setStatus] = useStored(`${model.id}.runs.status`, '')
  const [profile, setProfile] = useStored(`${model.id}.runs.profile`, '')
  const [selected, setSelected] = useState([])
  return e(Loading, { query }, data => {
    const pool = data.runs.filter(r => (!status || r.status === status) && (!profile || r.profile_id === profile))
    const found = searchItems(pool, search, RUN_FIELDS)
    return e('div', { style: stack(10) },
      e(ListSearch, { value: search, onChange: setSearch, shown: found.length, total: data.runs.length, noun: 'run', placeholder: 'Search runs: name, dataset, profile or status' },
        e(Pick, { label: 'Status', value: status, onChange: setStatus, options: STATUSES }),
        e(Pick, { label: 'Profile', value: profile, onChange: setProfile, options: [['', 'any'], ...model.profiles.map(p => [p.id, p.name])] }),
        selected.length >= 2 ? e(Button, { size: 'xs', variant: 'secondary', onClick: () => go(placeOf(model.id, 'runs', 'compare', selected.join(','))) }, `Compare ${selected.length}`)
          : e('span', { style: faint }, 'Tick 2 to 4 runs to compare them')),
      e(RunTable, { model, runs: found, selected, onSelect: r => setSelected(pickRuns(selected, r)),
        empty: data.runs.length ? e('div', { style: muted }, 'No run matches.')
          : e(Empty, { title: 'No runs yet', description: 'Open a dataset and solve it.', action: e(Button, { size: 'xs', onClick: () => go(placeOf(model.id, 'datasets')) }, 'Datasets') }) }))
  })
}

const LEVELS = [['', 'any level'], ['hard', 'hard'], ['medium', 'medium'], ['soft', 'soft']]

/** A KPI with its change since the run measured against. */
function KpiTile({ kpi, before }) {
  const change = before && before.value !== kpi.value ? kpi.value - before.value : null
  return e('div', { style: { ...card, padding: 10 } },
    e('div', { style: faint }, kpi.label),
    e('div', { style: { fontWeight: 600, fontSize: '1rem', fontVariantNumeric: 'tabular-nums' } }, `${fmt(kpi.value)} ${kpi.unit || ''}`),
    change !== null ? e('div', { style: faint }, `${change > 0 ? '+' : '−'}${fmt(Math.abs(change))} since the run before`) : null)
}

function RunPage({ model, id }) {
  const { docked } = useNav()
  const [open, setOpen] = useState(null)
  const [search, setSearch] = useStored(`${model.id}.run.search`, '')
  const [costlyOnly, setCostlyOnly] = useStored(`${model.id}.run.costly`, true)
  const [level, setLevel] = useStored(`${model.id}.run.level`, '')
  const [naming, setNaming] = useState(null)
  const [replanning, setReplanning] = useState(false)
  const [picked, setPicked] = useState(null)
  const query = useRead('run', { run: id, max_matches: 10 })
  const cancel = useAction('cancel')
  const rename = useAction('run_update', () => setNaming(null))
  const run0 = answer(query, 'run')
  useKeys({ r: () => run0 && FINISHED.includes(run0.status) && setReplanning(true) })
  return e(Loading, { query }, ({ run }) => {
    const live = LIVE.includes(run.status)
    const finished = FINISHED.includes(run.status)
    const all = run.analysis || []
    const costing = all.filter(c => parseScore(c.score).some(p => p.value !== 0))
    const catalog = new Map(model.constraints.map(c => [c.name, c]))
    const pool = (costlyOnly ? costing : all).map(c => ({ ...catalog.get(c.name), ...c })).filter(c => !level || c.level === level)
    const rows = searchConstraints(pool, search)
    const hard = brokenHard(run.score)
    const previous = run.previous
    const before = new Map(((previous && previous.kpis) || []).map(k => [k.id, k]))
    const analysis = all.length
      ? e(Card, null,
          e(Label, null, 'What costs score'),
          e(ListSearch, { value: search, onChange: setSearch, shown: rows.length, total: costlyOnly ? costing.length : all.length, noun: 'rule', placeholder: 'Search rules: name, group, level or what they mean' },
            e(Pick, { label: 'Level', value: level, onChange: setLevel, options: LEVELS }),
            e('label', { style: { ...row(4), ...faint } },
              e('input', { type: 'checkbox', checked: costlyOnly, onChange: event => setCostlyOnly(event.target.checked) }),
              `Only what costs score (${costing.length})`)),
          rows.length
            ? e(Table, {
                // The rule's name takes what the fixed columns leave; the score wraps by level.
                columns: [{ id: 'name', label: 'Rule' }, { id: 'level', label: 'Level', width: 64 },
                  { id: 'count', label: 'Matches', num: true, width: 72 }, { id: 'score', label: 'Score', num: true, width: '34%' }],
                rows: rows.map(({ item: c, hits }) => ({
                  key: c.name, name: e('div', null, e(Highlight, { text: c.name, hits }), open === c.name ? e(Matches, { item: c }) : null),
                  level: e('span', { style: { color: levelColor(c.level) } }, c.level),
                  count: fmt(c.match_count), score: e(ScoreText, { score: c.score })
                })),
                onRow: r => setOpen(open === r.key ? null : r.key)
              })
            : e('div', { style: muted }, costlyOnly && !search.trim() && !level ? 'Nothing costs score.' : 'No rule matches.'))
      : live ? e('div', { style: muted }, 'Solving…') : null
    const plan = model.view && run.status !== 'queued'
      ? e('div', { style: stack(8) },
          e('div', { style: row(8) }, e(H2, null, viewName(model.view)), spacer(),
            finished ? e('span', { style: faint }, 'Click an open item to find it a place') : null),
          e(PlanView, { run: run.id, kind: model.view, version: `${run.status}:${run.score}`, onItem: finished ? setPicked : undefined }))
      : null
    return e('div', { style: stack(14) },
      e('div', { style: row(10) },
        naming !== null
          ? e('form', { style: row(6), onSubmit: event => { event.preventDefault(); rename.mutate({ run: run.id, label: naming }) } },
              e('input', { value: naming, autoFocus: true, 'aria-label': 'Run name', style: { ...input, width: 260 }, onChange: event => setNaming(event.target.value),
                onKeyDown: event => { if (event.key === 'Escape') setNaming(null) } }),
              e(Button, { size: 'xs', type: 'submit', disabled: rename.isPending }, 'Save'),
              e(Button, { size: 'xs', variant: 'text', onClick: () => setNaming(null) }, 'Cancel'))
          : e('span', { style: row(6) }, e(H2, null, runName(run)),
              e(Button, { size: 'xs', variant: 'text', onClick: () => setNaming(run.label || '') }, 'Rename'),
              e(CopyId, { id: run.id, what: 'run' })),
        spacer(),
        finished ? e(Button, { size: 'xs', onClick: () => setReplanning(true) }, 'Replan…') : null,
        run.status !== 'queued' && !(run.status === 'invalid' || run.status === 'failed') && run.score
          ? e(ExportButton, { action: 'export', args: { run: run.id }, name: `${run.dataset_name || 'plan'} ${runName(run)}`, label: 'Export plan' }) : null,
        live ? e(Button, { size: 'xs', variant: 'outline', disabled: cancel.isPending, onClick: () => cancel.mutate({ run: run.id }) }, 'Stop') : null),
      e('div', { style: { ...row(8), ...muted } },
        e(Link, { to: placeOf(model.id, 'datasets', run.dataset_id), style: { textDecoration: 'underline' } }, run.dataset_name || 'its dataset'),
        e('span', null, `· ${profileLabel(run)} · ${ago(run.created_at)}${run.elapsed_ms != null && !live ? ` · ${duration(run.elapsed_ms)}` : ''}`)),
      e(LineageStrip, { model: model.id, lineage: run.lineage, here: run.dataset_name || 'this dataset' }),
      e(Card, null,
        e('div', { style: row(10) },
          e('strong', { style: { fontSize: '0.95rem', color: hard < 0 || run.status === 'failed' || run.status === 'invalid' ? DANGER : undefined } }, verdictText(run)),
          spacer(), e(Status, { status: run.status })),
        e('div', { style: row(10) },
          e(ScoreText, { score: run.score }),
          run.initial_score && run.initial_score !== run.score ? e('span', { style: { ...row(6), ...faint } }, 'the plan as given scored', e(ScoreText, { score: run.initial_score })) : null,
          previous ? e('span', { style: { ...row(6), ...faint } }, 'since', e(Link, { to: placeOf(model.id, 'runs', previous.id), style: { textDecoration: 'underline' } }, runName(previous)),
            e(DeltaText, { from: previous.score, to: run.score })) : null),
        run.error ? e('div', { role: 'alert', style: muted }, errorText(run.error),
          run.error.issues ? e('ul', { style: { margin: '4px 0', paddingLeft: 18 } }, run.error.issues.map((i, n) => e('li', { key: n }, i.message))) : null) : null,
        e(ScoreChart, { points: run.points })),
      finished ? e(ChangesCard, { run }) : null,
      run.kpis && run.kpis.length
        ? e('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(150px, 1fr))', gap: 10 } },
            run.kpis.map(k => e(KpiTile, { key: k.id, kpi: k, before: before.get(k.id) })))
        : null,
      // A map needs the page's width: a routing plan spans it, its rules below. Other
      // views share the page with the rules beside them.
      plan && analysis && !docked && model.view !== 'routes'
        ? e('div', { style: { display: 'grid', gridTemplateColumns: 'minmax(0, 3fr) minmax(0, 2fr)', gap: 14, alignItems: 'start' } }, plan, analysis)
        : [plan, analysis],
      finished ? e(FillPanel, { key: picked ? picked.id : 'all', run: run.id, model: model.id, items: picked ? [picked.id] : undefined, onClear: () => setPicked(null) }) : null,
      finished ? e(ReplanDialog, { model, run, open: replanning, onClose: () => setReplanning(false) }) : null)
  })
}

function ComparePage({ model, ids }) {
  const { docked } = useNav()
  const query = useRead('compare', { runs: ids })
  const [all, setAll] = useState(false)
  const [plans, setPlans] = useStored(`${model.id}.compare.plans`, true)
  return e(Loading, { query }, data => {
    const columns = [{ id: 'name', label: '' }, ...data.runs.map((r, i) => ({ id: `r${i}`, label: `${r.dataset} · ${r.profile || 'Standard'} · ${runName(r)}`, num: true }))]
    const line = (key, name, values) => ({ key, name, ...Object.fromEntries(values.map((v, i) => [`r${i}`, v])) })
    const differs = values => values.some(v => String(v) !== String(values[0]))
    const constraints = data.constraints.filter(c => all || differs(c.scores))
    return e('div', { style: stack(12) },
      e('div', { style: row(10) }, e(H2, null, 'Compare runs'), spacer(),
        e('label', { style: { ...row(4), ...faint } }, e('input', { type: 'checkbox', checked: !all, onChange: event => setAll(!event.target.checked) }), 'Only rules that differ')),
      e(Table, { columns, rows: [
        line('score', e('strong', null, 'Score'), data.runs.map(r => e(ScoreText, { score: r.score }))),
        line('time', 'Solve time', data.runs.map(r => duration(r.elapsed_ms))),
        ...data.kpis.map(k => line(`kpi:${k.id}`, k.label, k.values.map(v => (v == null ? '—' : `${fmt(v)} ${k.unit}`)))),
        ...constraints.map(c => line(`c:${c.name}`, c.name, c.scores.map(s => e(ScoreText, { score: s }))))
      ] }),
      !constraints.length ? e('div', { style: faint }, 'Every rule scores the same in these runs.') : null,
      model.view
        ? e('div', { style: stack(8) },
            e('label', { style: { ...row(4), ...faint } }, e('input', { type: 'checkbox', checked: plans, onChange: event => setPlans(event.target.checked) }), 'Show the plans side by side'),
            plans ? e('div', { style: { display: 'grid', gridTemplateColumns: docked ? '1fr' : `repeat(${Math.min(data.runs.length, 2)}, minmax(0, 1fr))`, gap: 12 } },
              data.runs.map(r => e('div', { key: r.id, style: stack(6) },
                e(Link, { to: placeOf(model.id, 'runs', r.id), style: { fontWeight: 600 } }, `${r.dataset} · ${r.profile || 'Standard'} · ${runName(r)}`),
                e(PlanView, { run: r.id, kind: model.view, version: r.score })))) : null)
        : null)
  })
}

// ---- profiles ----------------------------------------------------------------------

function WeightCell({ value, editable, onChange, changed }) {
  if (!editable) return e('span', null, fmt(value))
  return e('input', {
    type: 'number', min: 0, max: 1000000, step: 1, value, 'aria-label': 'Weight',
    style: { ...input, width: 80, textAlign: 'right', ...(changed ? { borderColor: 'var(--ui-accent)' } : {}) },
    onChange: event => onChange(Math.max(0, Math.min(1000000, Math.trunc(Number(event.target.value) || 0))))
  })
}

export function ParamCell({ param, value, onChange, changed }) {
  const ring = changed ? { borderColor: 'var(--ui-accent)' } : {}
  const label = param.label || param.name
  if (param.kind === 'boolean') {
    return e('input', { type: 'checkbox', checked: Boolean(value), 'aria-label': label, onChange: event => onChange(event.target.checked) })
  }
  if (param.kind === 'object' && param.fields && param.fields.length) return e(ObjectCell, { param, value, onChange, ring })
  if (param.kind === 'array' && param.fields && param.fields.length) return e(ListCell, { param, value, onChange, ring })
  if (param.kind === 'object' || param.kind === 'array' || param.kind === 'json') return e(JsonCell, { param, value, onChange, ring })
  if (param.kind === 'location') {
    const [lat, lng] = Array.isArray(value) ? value : ['', '']
    const part = (n, words) => e('input', {
      type: 'number', step: 'any', value: [lat, lng][n] ?? '', 'aria-label': `${label} ${words}`, style: { ...input, width: 96, textAlign: 'right', ...ring },
      onChange: event => {
        const v = Number(event.target.value)
        if (event.target.value !== '' && Number.isFinite(v)) onChange(n ? [Number(lat) || 0, v] : [v, Number(lng) || 0])
      }
    })
    return e('span', { style: row(4) }, part(0, 'latitude'), part(1, 'longitude'))
  }
  if (param.kind === 'date_time') {
    return e('input', { type: 'datetime-local', value: value ? String(value).slice(0, 16) : '', 'aria-label': label, style: { ...input, ...ring },
      onChange: event => onChange(event.target.value || undefined) })
  }
  if (param.options && param.options.length) {
    return e('select', { value: value ?? '', 'aria-label': label, style: { ...input, ...ring }, onChange: event => onChange(event.target.value) },
      param.options.map(o => e('option', { key: o.value, value: o.value }, o.label)))
  }
  if (param.kind === 'date') {
    return e('input', { type: 'date', value: value ?? '', 'aria-label': label, style: { ...input, ...ring }, onChange: event => onChange(event.target.value) })
  }
  if (param.kind === 'string' || param.kind === 'text') {
    return e('input', { value: value ?? '', 'aria-label': label, style: { ...input, width: 160, ...ring }, onChange: event => onChange(event.target.value) })
  }
  const whole = param.kind === 'integer' || param.kind === 'duration' || param.kind === 'minutes'
  return e('span', { style: row(4) },
    e('input', {
      type: 'number', step: whole ? 1 : 'any', value: value ?? '', 'aria-label': label, style: { ...input, width: 96, textAlign: 'right', ...ring },
      onChange: event => {
        const n = Number(event.target.value)
        if (event.target.value !== '' && Number.isFinite(n)) onChange(whole ? Math.trunc(n) : n)
      }
    }),
    e('span', { style: faint }, param.unit || (param.kind === 'duration' || param.kind === 'minutes' ? 'minutes' : '')))
}

/** An object setting: each of its fields with its own editor. */
function ObjectCell({ param, value, onChange, ring }) {
  const current = value && typeof value === 'object' && !Array.isArray(value) ? value : (param.default || {})
  return e('span', { style: { ...stack(4), ...ring } },
    param.fields.map(field => e('label', { key: field.name, style: { ...row(6), ...faint } },
      e('span', { style: { minWidth: 90 }, title: field.description }, field.label || field.name),
      e(ParamCell, { param: field, value: current[field.name] ?? field.default, onChange: inner => onChange({ ...current, [field.name]: inner }) }))))
}

/** A list setting: one editor per item, with add and remove. */
function ListCell({ param, value, onChange, ring }) {
  const items = Array.isArray(value) ? value : (param.default || [])
  const item = param.fields[0]
  const blank = item.kind === 'boolean' ? false : item.kind === 'object' ? {} : ['integer', 'number', 'duration', 'minutes'].includes(item.kind) ? 0
    : item.kind === 'location' ? [0, 0] : item.options && item.options.length ? item.options[0].value : ''
  return e('span', { style: { ...stack(4), ...ring } },
    items.length ? null : e('span', { style: faint }, 'None'),
    items.map((v, n) => e('span', { key: n, style: row(4) },
      e(ParamCell, { param: { ...item, label: `${item.label || item.name} ${n + 1}` }, value: v, onChange: inner => onChange(items.map((x, i) => i === n ? inner : x)) }),
      e(Button, { size: 'xs', variant: 'text', onClick: () => onChange(items.filter((_, i) => i !== n)) }, 'Remove'))),
    e('span', null, e(Button, { size: 'xs', variant: 'outline', onClick: () => onChange([...items, blank]) }, `Add ${(item.label || item.name).toLowerCase()}`)))
}

/** A list or object setting edited as JSON, kept only while it parses to the right shape. */
function JsonCell({ param, value, onChange, ring }) {
  const [text, setText] = useState(() => JSON.stringify(value ?? param.default ?? (param.kind === 'array' ? [] : {}), null, 1))
  const [bad, setBad] = useState(false)
  return e('span', { style: stack(3) },
    e('textarea', {
      value: text, rows: 3, 'aria-label': param.label || param.name, spellCheck: false,
      style: { ...input, width: 260, fontFamily: 'var(--font-mono, monospace)', ...ring, ...(bad ? { borderColor: 'var(--ui-danger, #e5534b)' } : {}) },
      onChange: event => {
        setText(event.target.value)
        try {
          const parsed = JSON.parse(event.target.value)
          const fits = param.kind === 'json' || (param.kind === 'array' ? Array.isArray(parsed) : parsed !== null && typeof parsed === 'object' && !Array.isArray(parsed))
          setBad(!fits)
          if (fits) onChange(parsed)
        } catch { setBad(true) }
      }
    }),
    bad ? e('span', { style: { ...faint, color: DANGER } }, param.kind === 'json' ? 'Write JSON.' : param.kind === 'array' ? 'Write a JSON list.' : 'Write a JSON object.') : null)
}

/** A setting's value in words: its option's label, its unit, an object's fields, a list's items. */
export function paramText(param, value) {
  if (value === null || value === undefined) return 'not set'
  if (param.kind === 'boolean') return value ? 'yes' : 'no'
  const option = (param.options || []).find(o => o.value === value)
  if (option) return option.label
  if (param.kind === 'object' && param.fields && param.fields.length) {
    return param.fields.map(f => `${(f.label || f.name).toLowerCase()} ${paramText(f, value[f.name] ?? f.default)}`).join(' · ')
  }
  if (Array.isArray(value)) return value.length ? value.map(v => paramText((param.fields || [])[0] || {}, v)).join(', ') : 'none'
  if (typeof value === 'object') return JSON.stringify(value)
  return typeof value === 'number' ? `${fmt(value)}${param.unit ? ` ${param.unit}` : ''}` : String(value)
}

function ImportProfile({ model }) {
  const { go } = useNav()
  const load = useAction('profile_import', r => r.profile && go(placeOf(model.id, 'profiles', r.profile.id)))
  return e(JsonFileButton, { label: 'Import', variant: 'text', onFile: file => {
    if (file.error) { host.notify({ kind: 'error', message: file.error }); return }
    if (!file.value || file.value.model_id !== model.id) {
      host.notify({ kind: 'error', message: file.value && file.value.model_id ? `That profile is for ${file.value.model_id}, not ${model.name}.` : errorText({ code: 'invalid_profile' }) })
      return
    }
    const taken = model.profiles.some(p => p.name === file.value.name)
    load.mutate({ document: file.value, ...(taken ? { name: `${file.value.name} (imported)` } : {}) })
  } })
}

function ProfilesTab({ model, picked }) {
  const { go } = useNav()
  const [search, setSearch] = useStored(`${model.id}.profiles.search`, '')
  const fallback = model.profiles.find(p => p.id === model.default_profile) || model.profiles[0]
  const profile = model.profiles.find(p => p.id === picked) || fallback
  const found = searchItems(model.profiles, search, [['name', 1], ['description', 0.5]])
  return e('div', { style: { display: 'grid', gridTemplateColumns: 'minmax(200px, 260px) minmax(0, 1fr)', gap: 16, alignItems: 'start' } },
    e('div', { style: stack(8) },
      e(ListSearch, { value: search, onChange: setSearch, shown: found.length, total: model.profiles.length, noun: 'profile' },
        e(ImportProfile, { model })),
      found.map(({ item: p, hits }) => e('button', {
        key: p.id, type: 'button', onClick: () => go(placeOf(model.id, 'profiles', p.id)),
        style: { ...card, ...stack(3), padding: 10, textAlign: 'left', cursor: 'pointer', background: 'none', color: 'inherit',
          ...(profile && p.id === profile.id ? { borderColor: 'var(--ui-accent)', background: 'color-mix(in srgb, var(--ui-accent) 7%, transparent)' } : {}) }
      }, e('strong', null, e(Highlight, { text: p.name, hits })),
        e('span', { style: faint }, [p.id === model.default_profile ? 'default' : null,
          p.read_only ? 'the model’s own weights' : count(Object.keys(p.weights || {}).length + Object.keys(p.params || {}).length, 'change'),
          p.updated_at && !p.read_only ? `edited ${ago(p.updated_at)}` : null].filter(Boolean).join(' · '))))),
    profile ? e(ProfileEditor, { key: `${profile.id}:${profile.revision}`, model, profile }) : null)
}

function ProfileEditor({ model, profile }) {
  const { go } = useNav()
  const [draft, setDraft] = useState({})
  const [draftParams, setDraftParams] = useState({})
  const [name, setName] = useState('')
  const [search, setSearch] = useStored(`${model.id}.rules.search`, '')
  const [section, setSection] = useStored(`${model.id}.profile.section`, 'rules')
  const save = useAction('profile_save', r => { setDraft({}); setDraftParams({}); setName(''); r.profile && go(placeOf(model.id, 'profiles', r.profile.id)) })
  const makeDefault = useAction('profile_default')
  const weights = { ...(profile.weights || {}), ...draft }
  const params = { ...(profile.params || {}), ...draftParams }
  const editable = model.weights
  const parameters = model.parameters || []
  const changed = Object.keys(draft).length > 0 || Object.keys(draftParams).length > 0
  const settings = { weights, params, solve: profile.solve }
  const [onlyDiff, setOnlyDiff] = useStored(`${model.id}.profile.only-diff`, false)
  const differs = c => (weights[c.name] ?? 1) !== 1
  const paramDiffers = p => p.name in params && JSON.stringify(params[p.name]) !== JSON.stringify(p.default)
  const weightDiffs = model.constraints.filter(differs).length
  const paramDiffs = (model.parameters || []).filter(paramDiffers).length
  const results = searchConstraints(onlyDiff ? model.constraints.filter(differs) : model.constraints, search)
  const groups = []
  for (const r of results) {
    const group = r.item.group || 'Rules'
    let found = groups.find(g => g.name === group)
    if (!found) groups.push((found = { name: group, items: [], size: model.constraints.filter(c => (c.group || 'Rules') === group).length }))
    found.items.push(r)
  }
  return e('div', { style: stack(12) },
    e('div', { style: row(8) },
      e(H2, null, profile.name),
      profile.id === model.default_profile ? e(Badge, { variant: 'muted' }, 'Default') : null,
      spacer(),
      profile.id !== model.default_profile ? e(Button, { size: 'xs', variant: 'text', onClick: () => makeDefault.mutate({ model: model.id, profile: profile.id }) }, 'Make default') : null,
      !changed ? e(ExportButton, { action: 'profile_export', args: { model: model.id, profile: profile.id }, name: `${model.name} ${profile.name}` }) : null,
      !changed && !profile.read_only ? e(DeleteButton, { action: 'profile_delete', args: { model: model.id, profile: profile.id }, what: 'this profile',
        onDone: () => go(placeOf(model.id, 'profiles')) }) : null,
      changed ? e(Button, { size: 'xs', variant: 'text', onClick: () => { setDraft({}); setDraftParams({}) } }, 'Discard') : null,
      changed ? e('input', { value: name, placeholder: 'New profile name', style: input, 'aria-label': 'New profile name', onChange: event => setName(event.target.value) }) : null,
      changed ? e(Button, { size: 'xs', variant: 'outline', disabled: !name.trim() || save.isPending, onClick: () => save.mutate({ model: model.id, name: name.trim(), settings }) }, 'Save as new') : null,
      changed && !profile.read_only ? e(Button, { size: 'xs', disabled: save.isPending, onClick: () => save.mutate({ model: model.id, name: profile.name, description: profile.description, profile: profile.id, expected_revision: profile.revision, settings }) }, 'Save') : null),
    profile.read_only ? e('div', { style: faint }, 'Standard keeps the model’s own weights. Change a value, then save it as a new profile.') : null,
    !profile.read_only || changed
      ? e('div', { style: row(10) },
          e('span', { style: muted }, weightDiffs || paramDiffs
            ? `Against Standard: ${count(weightDiffs, 'rule')} weighed differently · ${count(paramDiffs, 'setting')} changed`
            : 'The same as Standard'),
          e('label', { style: { ...row(4), ...faint } },
            e('input', { type: 'checkbox', checked: onlyDiff, onChange: event => setOnlyDiff(event.target.checked) }), 'Only what differs'),
          spacer(),
          !changed && !profile.read_only ? e(Button, { size: 'xs', variant: 'outline', onClick: () => go(placeOf(model.id, 'experiments', 'new', '', profile.id)) }, 'Try it in an experiment') : null)
      : null,
    !editable ? e('div', { style: faint }, 'This model has fixed weights; only its settings can change.') : null,
    e(TabBar, { value: parameters.length ? section : 'rules', onChange: setSection,
      tabs: [{ id: 'rules', label: 'Rules and weights', count: model.constraints.length }, ...(parameters.length ? [{ id: 'settings', label: 'Settings', count: parameters.length }] : [])] }),
    section === 'settings' && parameters.length
      ? e(Table, {
          columns: [{ id: 'name', label: 'Setting' }, { id: 'standard', label: 'Standard', num: true, width: 130 }, { id: 'value', label: 'This profile', width: 300 }],
          rows: parameters.filter(prm => !onlyDiff || paramDiffers(prm)).map(prm => ({
            key: prm.name,
            name: e('div', null, prm.label || prm.name, prm.description ? e('div', { style: faint }, prm.description) : null),
            standard: e('span', { style: faint }, paramText(prm, prm.default)),
            value: e(ParamCell, { param: prm, value: params[prm.name] ?? prm.default, changed: prm.name in (profile.params || {}) || prm.name in draftParams,
              onChange: v => setDraftParams({ ...draftParams, [prm.name]: v }) })
          }))
        })
      : e('div', { style: stack(10) },
          e(ListSearch, { value: search, onChange: setSearch, shown: results.length, total: model.constraints.length, noun: 'rule', placeholder: 'Search rules: name, group, level or what they mean' }),
          search.trim() && !results.length ? e('div', { style: muted }, `No rule matches “${search.trim()}”.`) : null,
          groups.map(g => e('div', { key: g.name, style: stack(6) },
            e('div', { style: { ...muted, fontWeight: 600 } }, search.trim() ? `${g.name} · ${g.items.length} of ${g.size}` : `${g.name} · ${g.size}`),
            e(Table, {
              columns: [{ id: 'name', label: 'Rule' }, { id: 'level', label: 'Level', width: 80 }, { id: 'weight', label: 'Weight', num: true, width: 110 }],
              rows: g.items.map(({ item: c, hits }) => ({
                key: c.name,
                name: e('div', null, e(Highlight, { text: c.name, hits }), c.description ? e('div', { style: faint }, c.description) : null),
                level: e('span', { style: { color: levelColor(c.level) } }, c.level),
                weight: e(WeightCell, { value: weights[c.name] ?? 1, editable, changed: c.name in (profile.weights || {}) || c.name in draft,
                  onChange: v => setDraft({ ...draft, [c.name]: v }) })
              }))
            })))))
}

// ---- experiments and trends ---------------------------------------------------------

function ExperimentTable({ experiments, empty, showModel = false }) {
  const { go } = useNav()
  return e(Table, {
    empty,
    columns: [{ id: 'name', label: 'Experiment' }, ...(showModel ? [{ id: 'model', label: 'Model' }] : []),
      { id: 'shape', label: 'Datasets × profiles × repeats' }, { id: 'progress', label: 'Solves', num: true }, { id: 'when', label: 'Started', num: true, width: 110 }],
    rows: experiments.map(({ item: x, hits }) => ({
      key: x.id, id: x.id, modelId: x.model_id,
      name: e('strong', null, e(Highlight, { text: x.name, hits })),
      model: e('span', { style: muted }, x.model_name || x.model_id),
      shape: e('span', { style: muted }, `${x.datasets.length} × ${x.profiles.length} × ${x.repeats}`),
      progress: x.done ? e(Status, { status: 'completed' }) : `${fmt(x.finished)} of ${fmt(x.runs)}`,
      when: e('span', { style: faint }, ago(x.created_at))
    })),
    onRow: r => go(placeOf(r.modelId, 'experiments', r.id))
  })
}

function ExperimentsTab({ model }) {
  const { go } = useNav()
  const query = useRead('experiments', { model: model.id })
  const [search, setSearch] = useStored(`${model.id}.experiments.search`, '')
  useKeys({ e: () => go(placeOf(model.id, 'experiments', 'new')) })
  return e(Loading, { query }, data => {
    const found = searchItems(data.experiments, search, [['name', 1]])
    return e('div', { style: stack(10) },
      e(ListSearch, { value: search, onChange: setSearch, shown: found.length, total: data.experiments.length, noun: 'experiment' },
        e(Button, { size: 'xs', onClick: () => go(placeOf(model.id, 'experiments', 'new')) }, 'New experiment')),
      e(ExperimentTable, { experiments: found,
        empty: data.experiments.length ? e('div', { style: muted }, 'No experiment matches.')
          : e(Empty, { title: 'No experiments yet', description: 'An experiment solves datasets under several profiles and ranks the profiles.',
              action: e(Button, { size: 'xs', onClick: () => go(placeOf(model.id, 'experiments', 'new')) }, 'New experiment') }) }))
  })
}

function Picks({ label, options, chosen, onChange }) {
  return e('fieldset', { style: { ...card, ...stack(6), margin: 0 } },
    e('legend', { style: faint }, label),
    options.length
      ? options.map(o => e('label', { key: o.id, style: { ...muted, ...row(6) } },
          e('input', { type: 'checkbox', checked: chosen.includes(o.id), onChange: () => onChange(chosen.includes(o.id) ? chosen.filter(c => c !== o.id) : [...chosen, o.id]) }),
          o.label))
      : e('div', { style: faint }, 'None yet.'))
}

function ExperimentNew({ model, datasets: initial, profiles: chosen }) {
  const { go } = useNav()
  const datasetQuery = useRead('datasets', { model: model.id })
  const [datasets, setDatasets] = useState(initial || [])
  // A profile opened from its editor is tried against Standard, the baseline it changes.
  const standard = (model.profiles.find(p => p.read_only) || {}).id
  const [profiles, setProfiles] = useState(() => chosen && chosen.length ? [...new Set([standard, ...chosen].filter(Boolean))] : [])
  const [repeats, setRepeats] = useState('2')
  const [seconds, setSeconds] = useState('30')
  const [judge, setJudge] = useState('')
  const [name, setName] = useState('')
  const create = useAction('experiment_create', r => r.experiment && go(placeOf(model.id, 'experiments', r.experiment.id)))
  const all = answer(datasetQuery, 'datasets') || []
  const total = datasets.length * profiles.length * Number(repeats || 0)
  return e('div', { style: stack(12) },
    e(H2, null, 'New experiment'),
    e('div', { style: muted }, 'Every dataset is solved under every profile, repeats times each. Profiles weigh rules differently, so each plan is scored again under one judging profile to rank them.'),
    e('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: 10 } },
      e(Picks, { label: 'Datasets', chosen: datasets, onChange: setDatasets, options: all.map(d => ({ id: d.id, label: `${d.name} · ${sizeText(d.size)}` })) }),
      e(Picks, { label: 'Profiles', chosen: profiles, onChange: setProfiles, options: model.profiles.map(p => ({ id: p.id, label: p.name })) })),
    e('div', { style: row(10) },
      e('input', { value: name, placeholder: 'Name', style: input, 'aria-label': 'Experiment name', onChange: event => setName(event.target.value) }),
      e('label', { style: { ...row(4), ...faint } }, 'Repeats', e('input', { type: 'number', min: 1, max: 20, value: repeats, style: { ...input, width: 56 }, 'aria-label': 'Repeats', onChange: event => setRepeats(event.target.value) })),
      e('label', { style: { ...row(4), ...faint } }, 'Seconds each', e('input', { type: 'number', min: 1, value: seconds, style: { ...input, width: 64 }, 'aria-label': 'Seconds per solve', onChange: event => setSeconds(event.target.value) })),
      e('label', { style: { ...row(4), ...faint } }, 'Judged by',
        e('select', { value: judge, style: input, 'aria-label': 'Judge profile', onChange: event => setJudge(event.target.value) },
          e('option', { value: '' }, 'Standard'),
          model.profiles.filter(p => !p.read_only).map(p => e('option', { key: p.id, value: p.id }, p.name)))),
      e(Button, { size: 'xs', disabled: !total || total > 200 || create.isPending,
        onClick: () => create.mutate({ datasets, profiles, repeats: Math.max(1, Math.trunc(Number(repeats) || 1)), ...(name.trim() ? { name: name.trim() } : {}),
          ...(judge ? { judge } : {}), ...(Number(seconds) > 0 ? { overrides: { solve: { seconds: Math.trunc(Number(seconds)) } } } : {}) }) },
        total ? `Start ${total} solve${total === 1 ? '' : 's'}` : 'Pick datasets and profiles')),
    total > 200 ? e('div', { role: 'alert', style: muted }, 'At most 200 solves per experiment.') : null)
}

/** What a profile changes against the model's own weights and settings, in a few words. */
export function profileChanges(model, profileId) {
  const profile = model.profiles.find(p => p.id === profileId)
  if (!profile) return 'a deleted profile'
  const params = new Map((model.parameters || []).map(p => [p.name, p]))
  const parts = [
    ...Object.entries(profile.weights || {}).filter(([, w]) => w !== 1).map(([name, w]) => `${name} ×${fmt(w)}`),
    ...Object.entries(profile.params || {}).map(([name, v]) => `${(params.get(name) || {}).label || name}: ${paramText(params.get(name) || {}, v)}`)
  ]
  if (!parts.length) return 'the model’s own weights'
  return parts.length > 3 ? `${parts.slice(0, 3).join(' · ')} and ${parts.length - 3} more` : parts.join(' · ')
}

function ExperimentPage({ model, id }) {
  const { go } = useNav()
  const query = useRead('experiment', { experiment: id })
  const cancel = useAction('experiment_cancel')
  return e(Loading, { query }, ({ experiment: x }) => {
    const finished = Object.entries(x.statuses).filter(([k]) => !LIVE.includes(k)).reduce((n, [, v]) => n + v, 0)
    const runs = Object.values(x.statuses).reduce((n, v) => n + v, 0)
    const cellOf = (dataset, profile) => x.cells.find(c => c.dataset_id === dataset && c.profile_id === profile)
    return e('div', { style: stack(12) },
      e('div', { style: row(10) }, e(H2, null, x.name), e(CopyId, { id: x.id, what: 'experiment' }),
        x.done ? e(Badge, { variant: 'success' }, 'Done') : e(Badge, null, `${finished} of ${runs} solves`),
        spacer(),
        !x.done ? e(Button, { size: 'xs', variant: 'outline', disabled: cancel.isPending, onClick: () => cancel.mutate({ experiment: x.id }) }, 'Stop') : null,
        e(DeleteButton, { action: 'experiment_delete', args: { experiment: x.id }, what: 'this experiment', onDone: () => go(placeOf(model.id, 'experiments')) })),
      e('div', { style: muted }, `${count(x.datasets.length, 'dataset')} × ${count(x.profiles.length, 'profile')} × ${x.repeats} · judged by ${x.judge_name} · ${ago(x.created_at)}`),
      e(Card, null, e(Label, null, 'Ranking'),
        e(Table, {
          columns: [{ id: 'place', label: '#', width: 32 }, { id: 'profile', label: 'Profile' }, { id: 'changes', label: 'What it changes' },
            { id: 'feasible', label: 'Feasible', num: true }, { id: 'avg', label: 'Average place', num: true }],
          rows: x.ranking.map(r => ({
            key: r.profile_id, place: r.place, profile: e('strong', null, r.profile),
            changes: e('span', { style: faint }, profileChanges(model, r.profile_id)),
            feasible: r.finished ? `${r.feasible} of ${r.finished}` : '—',
            avg: r.average_place == null ? '—' : r.average_place
          }))
        }),
        e('div', { style: faint }, 'Most feasible solves first, then the best average place across datasets: each dataset ranks the profiles by their best plan, scored by the judge.')),
      e(Card, null, e(Label, null, 'By dataset'),
        e(Table, {
          columns: [{ id: 'dataset', label: 'Dataset' }, ...x.profiles.map(p => ({ id: p, label: x.profile_names[p] || 'Profile' }))],
          rows: x.datasets.map(d => ({
            key: d, dataset: x.dataset_names[d] || 'Dataset',
            ...Object.fromEntries(x.profiles.map(p => {
              const c = cellOf(d, p)
              return [p, c && c.best
                ? e(Link, { to: placeOf(model.id, 'runs', c.best.run_id) },
                    e(ScoreText, { score: c.best.score }),
                    e('div', { style: faint }, `${c.feasible} of ${c.finished} feasible${c.mean_elapsed_ms ? ` · ${duration(c.mean_elapsed_ms)}` : ''}`))
                : e('span', { style: faint }, c ? `${c.finished} of ${c.runs} done` : '—')]
            }))
          }))
        })))
  })
}

/** One KPI over time: a line through every finished solve, oldest on the left, each solve a
 *  point that names its dataset, profile and time and opens its run. */
export function TrendLine({ kpi, solves = [], model, width = 520, height = 72 }) {
  const { go } = useNav()
  const values = kpi.points.map(p => p[1])
  if (values.length < 2) return e('div', { style: faint }, `${kpi.label}: ${values.length ? `${fmt(values[0])} ${kpi.unit || ''}` : 'no data'}`)
  const low = Math.min(...values)
  const high = Math.max(...values)
  const span = high - low || 1
  const padding = 6
  const x = i => padding + (i / (values.length - 1)) * (width - 2 * padding)
  const y = v => padding + ((high - v) / span) * (height - 2 * padding)
  const d = values.map((v, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join('')
  const byRun = new Map(solves.map(s => [s.run_id, s]))
  return e('figure', { style: { margin: 0 } },
    e('svg', { role: 'img', 'aria-label': `${kpi.label} over time`, viewBox: `0 0 ${width} ${height}`, width: '100%', height, preserveAspectRatio: 'none', style: { display: 'block', overflow: 'visible' } },
      e('path', { d, fill: 'none', stroke: 'var(--ui-accent)', strokeWidth: 1.5, vectorEffect: 'non-scaling-stroke' }),
      kpi.points.map(([when, v, runId], i) => {
        const solve = byRun.get(runId)
        const words = `${fmt(v)} ${kpi.unit || ''} · ${solve ? `${solve.dataset} · ${solve.profile || 'Standard'} · ` : ''}${ago(when)}`
        const open = runId && model ? () => go(placeOf(model, 'runs', runId)) : undefined
        return e('circle', { key: `${runId}-${i}`, cx: x(i), cy: y(v), r: 3.5, fill: solve && !solve.feasible ? 'var(--ui-danger, #e5534b)' : 'var(--ui-accent)',
          style: { cursor: open ? 'pointer' : 'default' }, vectorEffect: 'non-scaling-stroke', onClick: open,
          ...(open ? { tabIndex: 0, role: 'link', 'aria-label': `Open the run: ${words}`,
            onKeyDown: event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); open() } } } : {}) }, e('title', null, words))
      })),
    e('figcaption', { style: CAPTION },
      e('span', null, `${ago(kpi.points[0][0])} · ${fmt(values[0])}`),
      e('span', null, `${kpi.label} · ${count(values.length, 'solve')} · range ${fmt(low)}–${fmt(high)} ${kpi.unit || ''}`),
      e('span', null, `${fmt(values[values.length - 1])} · ${ago(kpi.points[kpi.points.length - 1][0])}`)))
}

function TrendsTab({ model }) {
  const query = useRead('trends', { model: model.id })
  return e(Loading, { query }, data => e('div', { style: stack(14) },
    data.solves.length
      ? e('div', { style: muted }, `${count(data.solves.length, 'finished solve')} · ${Math.round(data.feasible_share * 100)}% feasible · ${ago(data.solves[0].finished_at)} to ${ago(data.solves[data.solves.length - 1].finished_at)}`)
      : e(Empty, { title: 'No finished solves yet', description: 'Trends appear once this model has solved plans.' }),
    data.kpis.map(k => e(Card, { key: k.id }, e(TrendLine, { kpi: k, solves: data.solves, model: model.id })))))
}

// ---- across models -------------------------------------------------------------------

function useModelNames() {
  const models = useRead('models')
  return new Map((answer(models, 'models') || []).map(m => [m.id, m.name]))
}

function AllRuns() {
  const query = useRead('runs', {})
  const names = useModelNames()
  const [search, setSearch] = useStored('runs.search', '')
  const [status, setStatus] = useStored('runs.status', '')
  const [model, setModel] = useStored('runs.model', '')
  return e(Loading, { query }, data => {
    const pool = data.runs.filter(r => (!status || r.status === status) && (!model || r.model_id === model))
      .map(r => ({ ...r, model_name: names.get(r.model_id) || r.model_id }))
    const ordered = [...pool.filter(r => LIVE.includes(r.status)), ...pool.filter(r => !LIVE.includes(r.status))]
    const found = searchItems(ordered, search, [...RUN_FIELDS, ['model_name', 0.5]])
    return e('div', { style: stack(12) },
      e(H1, null, 'All runs'),
      e(ListSearch, { value: search, onChange: setSearch, shown: found.length, total: data.runs.length, noun: 'run', placeholder: 'Search runs: name, model, dataset, profile or status' },
        e(Pick, { label: 'Model', value: model, onChange: setModel, options: [['', 'any'], ...names.entries()] }),
        e(Pick, { label: 'Status', value: status, onChange: setStatus, options: STATUSES })),
      e(RunTable, { runs: found, showModel: true, empty: e('div', { style: muted }, data.runs.length ? 'No run matches.' : 'No runs yet.') }))
  })
}

function AllExperiments() {
  const { go } = useNav()
  const query = useRead('experiments', {})
  const names = useModelNames()
  const [search, setSearch] = useStored('experiments.search', '')
  const [model, setModel] = useStored('experiments.model', '')
  return e(Loading, { query }, data => {
    const pool = data.experiments.filter(x => !model || x.model_id === model).map(x => ({ ...x, model_name: names.get(x.model_id) || x.model_id }))
    const found = searchItems(pool, search, [['name', 1], ['model_name', 0.6]])
    return e('div', { style: stack(12) },
      e(H1, null, 'All experiments'),
      e(ListSearch, { value: search, onChange: setSearch, shown: found.length, total: data.experiments.length, noun: 'experiment' },
        e(Pick, { label: 'Model', value: model, onChange: setModel, options: [['', 'any'], ...names.entries()] }),
        model ? e(Button, { size: 'xs', onClick: () => go(placeOf(model, 'experiments', 'new')) }, 'New experiment')
          : e('span', { style: faint }, 'Pick a model to start a new experiment')),
      e(ExperimentTable, { experiments: found, showModel: true,
        empty: e('div', { style: muted }, data.experiments.length ? 'No experiment matches.' : 'No experiments yet.') }))
  })
}

/** Opens a run, dataset or experiment known only by its id, at its place in its model. */
function Find({ kind, id }) {
  const { go } = useNav()
  const query = useRead(kind, { [kind]: id })
  const record = answer(query, kind)
  useEffect(() => {
    const tab = { run: 'runs', dataset: 'datasets', experiment: 'experiments' }[kind]
    if (record) go(placeOf(record.model_id, tab, record.id), { replace: true })
  }, [record && record.id])
  return e(Loading, { query }, () => e(Skeleton, { style: { height: 18, width: '40%' } }))
}

// ---- the page --------------------------------------------------------------------------

function Breadcrumbs({ place }) {
  const inModel = place.page === 'model'
  const model = useRead('model', { model: inModel ? place.model : '' }, { enabled: inModel })
  const wanted = kind => inModel && place.tab === `${kind}s` && Boolean(place.id) && !['compare', 'new'].includes(place.id)
  const dataset = useRead('dataset', { dataset: wanted('dataset') ? place.id : '' }, { enabled: wanted('dataset') })
  const run = useRead('run', { run: wanted('run') ? place.id : '', max_matches: 0 }, { enabled: wanted('run') })
  const experiment = useRead('experiment', { experiment: wanted('experiment') ? place.id : '' }, { enabled: wanted('experiment') })
  const m = answer(model, 'model')
  const items = [{ label: 'SolverForge Workbench', to: '/home' }]
  if (place.page === 'runs') items.push({ label: 'All runs' })
  if (place.page === 'experiments') items.push({ label: 'All experiments' })
  if (place.page === 'add') items.push({ label: 'Add your model' })
  if (inModel) {
    items.push({ label: (m && m.name) || 'Model', to: placeOf(place.model) })
    const tab = MODEL_TABS.find(([id]) => id === place.tab)
    if (tab && place.tab !== 'overview') items.push({ label: tab[1], to: placeOf(place.model, place.tab) })
    const r = answer(run, 'run')
    if (wanted('run') && r) items.push({ label: r.dataset_name || 'Dataset', to: placeOf(place.model, 'datasets', r.dataset_id) })
    if (wanted('run')) items.push({ label: r ? runName(r) : 'Run' })
    if (place.tab === 'runs' && place.id === 'compare') items.push({ label: 'Compare' })
    if (wanted('dataset')) items.push({ label: (answer(dataset, 'dataset') || {}).name || 'Dataset' })
    if (place.tab === 'experiments' && place.id === 'new') items.push({ label: 'New experiment' })
    if (wanted('experiment')) items.push({ label: (answer(experiment, 'experiment') || {}).name || 'Experiment' })
    if (place.tab === 'profiles' && place.id && m) {
      const p = m.profiles.find(x => x.id === place.id)
      if (p) items.push({ label: p.name })
    }
  }
  return e(Crumbs, { items })
}

/** The workbench at one place. `start` fixes the first place (tests, the pane beside the
 *  chat); otherwise the place follows the page address and the last one is remembered. */
export function Workbench({ start, docked = false }) {
  const page = start === undefined
  const [own, setOwn] = useState(() => start || storage.get(`${profileName()}:at`, '/home'))
  const [address, setAddress] = useState(() => (page ? readAt() : ''))
  useEffect(() => {
    if (!page || typeof window === 'undefined') return undefined
    const follow = () => setAddress(readAt())
    window.addEventListener('hashchange', follow)
    return () => window.removeEventListener('hashchange', follow)
  }, [page])
  // On the bare section address the remembered place shows without adding a history entry.
  const at = page && address ? address : own
  useEffect(() => { if (page) storage.set(`${profileName()}:at`, at) }, [at, page])
  const go = (next, options = {}) => {
    if (!page) { setOwn(next); return }
    if (options.replace && typeof window !== 'undefined' && window.history && window.history.replaceState) {
      window.history.replaceState(window.history.state, '', `#${hrefOf(next)}`)
    } else {
      host.navigate(hrefOf(next))
    }
    setAddress(next)
  }
  const place = parseAt(at)
  const scroller = useRef(null)
  useEffect(() => { if (scroller.current) scroller.current.scrollTop = 0 }, [at])
  useEffect(() => {
    if (!page || typeof window === 'undefined') return undefined
    let pending = false
    const onKey = event => {
      const t = event.target
      if (event.metaKey || event.ctrlKey || event.altKey || (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName)))) return
      if (!scroller.current || !scroller.current.isConnected || !scroller.current.offsetParent) return
      if (pending) {
        pending = false
        const tab = { o: 'overview', d: 'datasets', r: 'runs', p: 'profiles', e: 'experiments', t: 'trends' }[event.key]
        if (tab && place.page === 'model') { event.preventDefault(); go(placeOf(place.model, tab)) }
        if (event.key === 'h') { event.preventDefault(); go('/home') }
        return
      }
      if (event.key === 'g') { pending = true; return }
      if (event.key === '/') { event.preventDefault(); focusSearch(); return }
      if (runKey(event.key)) event.preventDefault()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
  const body = {
    home: () => e(Home, null),
    runs: () => e(AllRuns, null),
    experiments: () => e(AllExperiments, null),
    model: () => e(ModelHub, { key: place.model, place }),
    add: () => e(AddModel, null),
    find: () => e(Find, { key: `${place.kind}:${place.id}`, kind: place.kind, id: place.id })
  }[place.page]
  return e(Nav.Provider, { value: { at, go, docked } },
    e('div', { ref: scroller, style: { ...stack(14), padding: docked ? 12 : '16px 24px 32px', height: '100%', overflow: 'auto', color: 'var(--ui-text-primary)', boxSizing: 'border-box' } },
      e(Breadcrumbs, { place }),
      e(EngineNotice, null),
      body()))
}

function EngineNotice() {
  const query = useRead('status')
  const engine = query.data && query.data.ok ? query.data.engine : null
  if (!engine || engine.installed) return null
  return e(EmptyState, { title: 'The SolverForge engine is not installed', description: 'Type /solverforge-workbench install in the chat; it downloads a checksum-verified binary.' })
}

// ---- entry points ----------------------------------------------------------------------

/** Shows a place in the SolverForge Workbench section ('/m/<model>/runs/<run>', '/run/<id>', …). */
export function openPage(at = '/home') {
  host.navigate(hrefOf(at))
}

export function openBeside(at = '/home') {
  if (typeof host.openWorkspace !== 'function') {
    openPage(at)
    return
  }
  // Re-opening the same id fronts the tab and renders the new place.
  host.openWorkspace(`${ID}:workbench`, {
    title: 'SolverForge Workbench', minWidth: '28rem', dock: { pane: 'workspace', pos: 'right' },
    render: () => e(Workbench, { start: at, docked: true, key: at })
  })
}

export function StatusChip() {
  const query = useRead('status', {})
  const data = query.data && query.data.ok ? query.data : null
  if (!data || !(data.solving || data.queued)) return null
  const label = `⚙ ${data.solving} solving${data.queued ? ` · ${data.queued} waiting` : ''}`
  return e('button', {
    type: 'button', title: 'Show the SolverForge Workbench runs', onClick: () => openPage('/runs'),
    style: { ...tone('--ui-accent', '0.6875rem'), padding: '0 6px', height: '100%', background: 'none', border: 0, cursor: 'pointer' }
  }, label)
}

const RUN_ID = /^[0-9a-f]{12}$/

/** The ::solverforge-workbench{run="…"} card in the transcript: the run in brief. */
function RunCard({ attrs, streaming }) {
  const id = attrs && (attrs.run || attrs.id)
  // Untrusted attributes: render nothing rather than guess.
  if (!RUN_ID.test(id || '') || streaming) return null
  return e('section', { style: { maxWidth: 720, ...card } }, e(RunBrief, { id }))
}

export function RunBrief({ id }) {
  const query = useRead('run', { run: id, max_matches: 0 })
  return e(Loading, { query }, ({ run }) => {
    const costing = (run.analysis || []).filter(c => parseScore(c.score).some(p => p.value !== 0)).slice(0, 6)
    return e('div', { style: stack(8), 'data-run': run.id },
      e('div', { style: row(8) }, e('strong', null, `${run.dataset_name || 'Dataset'} · ${runName(run)}`), spacer(), e(Status, { status: run.status })),
      e('div', { style: row(8) }, run.feasible === 1 ? e(Badge, { variant: 'success' }, 'Feasible') : run.feasible === 0 ? e(Badge, { variant: 'destructive' }, 'Breaks hard rules') : null, e(ScoreText, { score: run.score })),
      costing.length
        ? e(Table, { columns: [{ id: 'name', label: 'What costs score' }, { id: 'score', label: 'Score', num: true, width: 220 }],
            rows: costing.map(c => ({ key: c.name, name: c.name, score: e(ScoreText, { score: c.score }) })) })
        : null,
      e('div', null, e(Button, { size: 'xs', variant: 'text', onClick: () => openPage(placeOf(run.model_id, 'runs', run.id)) }, 'Open in the SolverForge Workbench')))
  })
}

const PAGE_KEYS = [
  ['search', 'search this list', () => focusSearch()],
  ['new-dataset', 'new dataset', () => runKey('n')],
  ['solve', 'solve this dataset', () => runKey('s')],
  ['new-experiment', 'new experiment', () => runKey('e')],
  ['replan', 'replan this run or dataset', () => runKey('r')]
]

export default {
  id: ID,
  name: 'SolverForge Workbench',
  description: 'Optimization models: datasets, profiles, runs, fill, replanning, experiments and trends, with each plan explained.',
  register(ctx) {
    rest = (path, opts) => ctx.rest(path, opts)
    if (ctx.storage) storage = ctx.storage
    if (ctx.os) os = ctx.os
    if (ctx.files) files = ctx.files
    // Live updates come as events from the backend's watcher; nothing here polls.
    const reread = kinds => queryClient.invalidateQueries({ predicate: q => q.queryKey[0] === ID && kinds.includes(q.queryKey[1]) })
    ctx.onEvent(`plugin.${ID}.changed`, () => queryClient.invalidateQueries({ queryKey: [ID] }))
    ctx.onEvent(`plugin.${ID}.run_progress`, () => reread(['run', 'runs', 'status', 'dataset', 'experiment', 'experiments']))
    ctx.onEvent(`plugin.${ID}.build`, () => reread(['model_build']))
    ctx.registerMany([
      { id: 'run-card', area: TRANSCRIPT_DIRECTIVE_AREA, data: { name: 'solverforge-workbench', render: props => jsx(RunCard, props) } },
      { id: 'page', area: ROUTES_AREA, title: 'SolverForge Workbench', data: { path: BASE }, render: () => jsx(Workbench, {}) },
      { id: 'nav', area: SIDEBAR_NAV_AREA, data: { path: BASE, label: 'SolverForge Workbench', codicon: 'beaker' } },
      { id: 'chip', area: STATUSBAR_AREAS.right, order: 127, render: () => jsx(StatusChip, {}) },
      { id: 'open', area: PALETTE_AREA, data: { id: `${ID}.open`, label: 'SolverForge Workbench: models', keywords: ['optimize', 'solver', 'models', 'schedule'], run: () => openPage('/home') } },
      { id: 'runs', area: PALETTE_AREA, data: { id: `${ID}.runs`, label: 'SolverForge Workbench: all runs', keywords: ['solve', 'runs', 'score'], run: () => openPage('/runs') } },
      { id: 'experiments', area: PALETTE_AREA, data: { id: `${ID}.experiments`, label: 'SolverForge Workbench: all experiments', keywords: ['experiment', 'compare', 'profiles'], run: () => openPage('/experiments') } },
      { id: 'beside', area: PALETTE_AREA, data: { id: `${ID}.beside`, label: 'SolverForge Workbench: open beside the chat', keywords: ['optimize', 'solver', 'side', 'pane'], run: () => openBeside(storage.get(`${profileName()}:at`, '/home')) } },
      // The page's own actions, rebindable in Settings › Keyboard. On the page they also answer
      // to single keys while no field has focus: / n s e, and g then o d r p e t h.
      ...PAGE_KEYS.map(([key, label, run]) => ({ id: `key-${key}`, area: KEYBINDS_AREA, data: { id: `${ID}.${key}`, label: `SolverForge Workbench: ${label}`, defaults: [], run } }))
    ])
  }
}
