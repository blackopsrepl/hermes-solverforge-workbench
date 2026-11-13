// Constraint search over the employee scheduling model's real catalog (tests/fixtures/ess-catalog.json).
import assert from 'node:assert/strict'
import { cpSync, mkdirSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
mkdirSync(join(here, '.build'), { recursive: true })
cpSync(join(here, '../../desktop/plugin.js'), join(here, '.build/search.mjs'))
const { fuzzyMatch, searchConstraints } = await import('./.build/search.mjs')
const catalog = JSON.parse(readFileSync(join(here, '..', 'fixtures', 'ess-catalog.json'), 'utf8'))
const names = query => searchConstraints(catalog, query).map(r => r.item.name)

test('an empty query keeps every constraint in catalog order', () => {
  assert.equal(catalog.length, 127)
  assert.deepEqual(names(''), catalog.map(c => c.name))
  assert.deepEqual(names('   '), catalog.map(c => c.name))
})

test('a word finds its constraint and nothing that merely shares letters', () => {
  assert.deepEqual(names('overlap'), ['Overlapping shifts'])
  assert.deepEqual(names('night'), [])
  assert.deepEqual(names('zzzq'), [])
})

test('abbreviations find the constraint they shorten, best first', () => {
  assert.deepEqual(names('rst btwn').slice(0, 2), ['Rest between shifts outside required range', 'Rest between shifts outside preferred range'])
  assert.equal(names('shft cnt')[0], 'Shift count per period outside required range')
  assert.deepEqual(names('consec days off').slice(0, 2), ['Consecutive days off per period below required minimum', 'Consecutive days off per period below preferred minimum'])
})

test('every term must match: by group, by level, or by name', () => {
  const travel = searchConstraints(catalog, 'hard travel').map(r => r.item)
  assert.ok(travel.length > 0)
  assert.ok(travel.every(c => c.level === 'hard' && c.group === 'Travel and locations'))
  assert.deepEqual(names('fair').slice(0, 2), ['Uneven shift counts', 'Uneven time worked'])
  assert.ok(names('weekend preferred').every(n => /weekend/i.test(n) && /preferred/i.test(n)))
})

test('matches in the name are highlighted at the letters that matched', () => {
  assert.deepEqual(searchConstraints(catalog, 'overlap')[0].hits, [0, 1, 2, 3, 4, 5, 6])
  const [rest] = searchConstraints(catalog, 'rst btwn')
  assert.equal(rest.hits.map(i => rest.item.name[i]).join(''), 'Rstbtwn')
  assert.equal(fuzzyMatch('cdo', 'Consecutive days off').hits.map(i => 'Consecutive days off'[i]).join(''), 'Cdo')
  assert.equal(fuzzyMatch('ab', 'xaxb'), null)
})
