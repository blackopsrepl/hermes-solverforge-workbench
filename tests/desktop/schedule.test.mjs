// The roster page the backend serves, run in a DOM: solverforge-ui must accept the model and draw it.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { JSDOM, VirtualConsole } from 'jsdom'

const here = dirname(fileURLToPath(import.meta.url))
const { document: page } = JSON.parse(readFileSync(join(here, 'fixtures', 'view.json'), 'utf8'))
const run = JSON.parse(readFileSync(join(here, '..', 'fixtures', 'ess-small-week-run.json'), 'utf8'))

function draw(html) {
  const errors = []
  const virtualConsole = new VirtualConsole()
  virtualConsole.on('jsdomError', error => errors.push(error.message))
  const dom = new JSDOM(html, { runScripts: 'dangerously', pretendToBeVisual: true, virtualConsole })
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
  const broken = page.replace('"startMinute": 0, "endMinute": 10080', '"startMinute": 0.5, "endMinute": 10080')
  assert.notEqual(broken, page)
  const { doc } = draw(broken)
  assert.match(doc.querySelector('#view').textContent, /The schedule could not be drawn/)
})
