// A plan frame that follows a live solve: the shown drawing stays until the next one has loaded.
import assert from 'node:assert/strict'
import { cpSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { JSDOM } from 'jsdom'

const dom = new JSDOM('<!doctype html><div id="root"></div>', { pretendToBeVisual: true })
globalThis.window = dom.window
globalThis.document = dom.window.document
globalThis.IS_REACT_ACT_ENVIRONMENT = true

const here = dirname(fileURLToPath(import.meta.url))
mkdirSync(join(here, '.build'), { recursive: true })
cpSync(join(here, '../../desktop/plugin.js'), join(here, '.build/plugin-frame.mjs'))
const { SteadyFrame } = await import('./.build/plugin-frame.mjs')
const { createElement: h, act } = await import('react')
const { createRoot } = await import('react-dom/client')

const page = text => 'data:text/html;charset=utf-8,' + encodeURIComponent(`<p>${text}</p>`)
const frames = () => [...document.querySelectorAll('iframe')]
const hidden = node => node.style.visibility === 'hidden'
// jsdom loads every frame at once; here a frame loads only when the test says so.
document.addEventListener('load', event => { if (event.isTrusted) event.stopPropagation() }, true)

test('the next plan loads hidden behind the shown one and replaces it once loaded', async () => {
  const heights = []
  const root = createRoot(document.getElementById('root'))
  const draw = src => act(() => root.render(h(SteadyFrame, { src, title: 'Schedule', onHeight: x => heights.push(x), style: { height: 300 } })))
  await draw(page('first'))
  assert.equal(frames().length, 1)
  const first = frames()[0]
  assert.ok(!hidden(first))

  await draw(page('second'))
  assert.equal(frames().length, 2, 'the first plan stays while the second loads')
  const [shown, next] = frames()
  assert.equal(shown, first)
  assert.ok(!hidden(shown) && hidden(next))
  assert.match(next.dataset.src, /second/)

  // The page reports its height while still hidden; it counts once it is shown.
  await act(() => window.dispatchEvent(new window.MessageEvent('message', {
    data: { source: 'solverforge-workbench', height: 412.2 }, source: next.contentWindow })))
  assert.deepEqual(heights, [])

  await act(() => next.dispatchEvent(new window.Event('load')))
  assert.deepEqual(frames(), [next], 'the loaded plan takes the place of the old one without reloading')
  assert.ok(!hidden(next))
  assert.deepEqual(heights, [413])

  // A plan that changes again before the next one loaded replaces only the pending one.
  await draw(page('third'))
  await draw(page('fourth'))
  assert.equal(frames().length, 2)
  assert.equal(frames()[0], next)
  assert.match(frames()[1].dataset.src, /fourth/)
  await act(() => root.unmount())
})
