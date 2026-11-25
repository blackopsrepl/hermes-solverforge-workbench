// A stand-in for the Desktop SDK: components render as plain elements that keep their text,
// and queries are answered from the fixture the test sets. Only what plugin.js imports.
import { createElement as h } from 'react'

let fixture = {}
export const opened = []
export const navigated = []
export const setFixture = value => {
  fixture = value
  opened.length = 0
  navigated.length = 0
}

const box = name => ({ children, title, description, variant }) =>
  h('div', { 'data-c': name, 'data-variant': variant }, title, description, children)

export const Badge = ({ children, variant }) => h('span', { 'data-c': 'Badge', 'data-variant': variant }, children)
export const EmptyState = box('EmptyState')
export const SandboxedFrame = ({ src, title, ref, style, onLoad }) => h('iframe', { title, 'data-length': src.length, 'data-src': src.slice(-24), ref, style, onLoad })
export const Button = ({ children, disabled }) => h('button', { disabled }, children)
export const ErrorState = box('ErrorState')
export const Skeleton = () => h('div', { 'data-c': 'Skeleton' })
export const CopyButton = ({ text, label }) => h('button', { 'data-c': 'CopyButton', 'data-text': text, 'aria-label': label })
export const SearchField = ({ placeholder, value }) => h('input', { placeholder, value, readOnly: true })
export const Dialog = ({ open, children }) => (open ? h('div', { 'data-c': 'Dialog' }, children) : null)
export const DialogContent = box('DialogContent')
export const DialogHeader = box('DialogHeader')
export const DialogTitle = box('DialogTitle')
export const DialogDescription = box('DialogDescription')
export const ConfirmDialog = ({ open, title, description, confirmLabel }) =>
  open ? h('div', { 'data-c': 'ConfirmDialog' }, title, description, confirmLabel) : null

export const KEYBINDS_AREA = 'keybinds'
export const PALETTE_AREA = 'palette'
export const ROUTES_AREA = 'routes'
export const SIDEBAR_NAV_AREA = 'sidebar'
export const TRANSCRIPT_DIRECTIVE_AREA = 'transcript'
export const STATUSBAR_AREAS = { right: 'statusbar-right' }

export const queryClient = { invalidateQueries() {} }
export const useMutation = () => ({ mutate() {}, mutateAsync: async () => ({}), isPending: false })
export const host = {
  state: { profile: { get: () => 'default' } },
  notify() {},
  navigate: path => navigated.push(path),
  openWorkspace: (id, options) => {
    opened.push({ id, ...options })
    return () => {}
  }
}

// The query key is [plugin id, read name, ...query pairs]; fixtures are keyed by read name.
export const useQuery = ({ queryKey, enabled = true }) => ({ data: enabled ? fixture[queryKey[1]] : undefined, isLoading: false })
