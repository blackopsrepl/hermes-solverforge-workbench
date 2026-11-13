// A stand-in for the Desktop SDK: components render as plain elements that keep their text,
// and queries are answered from the fixture the test sets. Only what plugin.js imports.
import { createElement as h } from 'react'

let fixture = {}
export const opened = []
export const setFixture = value => {
  fixture = value
  opened.length = 0
}

const box = name => ({ children, title, description, variant }) =>
  h('div', { 'data-c': name, 'data-variant': variant }, title, description, children)

export const Badge = ({ children, variant }) => h('span', { 'data-c': 'Badge', 'data-variant': variant }, children)
export const EmptyState = box('EmptyState')
export const Button = ({ children, disabled }) => h('button', { disabled }, children)
export const SegmentedControl = ({ options, value }) =>
  h('nav', null, options.map(o => h('b', { key: o.id, 'data-on': o.id === value }, o.label)))

export const PALETTE_AREA = 'palette'
export const ROUTES_AREA = 'routes'
export const SIDEBAR_NAV_AREA = 'sidebar'
export const TRANSCRIPT_DIRECTIVE_AREA = 'transcript'
export const STATUSBAR_AREAS = { right: 'statusbar-right' }

export const queryClient = { invalidateQueries() {} }
export const useMutation = () => ({ mutate() {}, isPending: false })
export const host = {
  notify() {},
  navigate() {},
  openWorkspace: (id, options) => {
    opened.push({ id, ...options })
    return () => {}
  }
}

// The query key is [plugin id, read name, ...query pairs]; fixtures are keyed by read name.
export const useQuery = ({ queryKey }) => ({ data: fixture[queryKey[1]], isLoading: false })
