import resolveConfig from 'tailwindcss/resolveConfig.js'
import contextUtils from 'tailwindcss/lib/lib/setupContextUtils.js'
import config from '../tailwind.config.js'
import { colorTokens, utilityTokens } from './design-token-map.mjs'

const escape = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
const aliases = Object.entries(utilityTokens).map(([oldName, name]) => [name, oldName])
  .sort(([a], [b]) => b.length - a.length)
const colors = Object.fromEntries(Object.entries(colorTokens).map(([oldName, name]) => [name, oldName]))
colors.warn = colors.accent
const colorPattern = new RegExp(`(?<![\\w-])((?:bg|text|border(?:-[trblxyse])?|ring(?:-offset)?|accent|from|via|to|divide|placeholder|decoration|fill|stroke|outline)-)(${Object.keys(colors).sort((a, b) => b.length - a.length).map(escape).join('|')})(?=/|$)`, 'g')

function legacyName(candidate) {
  let result = candidate
  for (const [name, oldName] of aliases) {
    result = result.replace(new RegExp(`(?<![\\w-])${escape(name)}(?![\\w-])`, 'g'), () => oldName)
  }
  return result.replace(colorPattern, (_, prefix, name) => prefix + colors[name])
}

// Renaming utility classes changes Tailwind's lexical tie-breaker. Preserve the
// original cascade (including overlapping className overrides) through the rework.
// Tailwind v3's own ordering API supplies property and variant precedence.
export default function designUtilityOrder() {
  return {
    postcssPlugin: 'ro-design-utility-order',
    OnceExit(root) {
      const context = contextUtils.createContext(resolveConfig({
        ...config,
        theme: {
          ...config.theme,
          extend: {
            ...config.theme.extend,
            boxShadow: {
              ...config.theme.extend.boxShadow,
              glass: 'var(--shadow-panel)',
              'glow-amber': 'var(--shadow-glow-warn)',
              'glow-emerald': 'var(--shadow-glow-ok)',
              'glow-red': 'var(--shadow-glow-bad)',
            },
          },
        },
      }))
      const candidates = new Set()
      root.walk((node) => {
        const meta = node.raws.tailwind
        if (meta?.layer === 'utilities' || meta?.parentLayer === 'utilities') {
          if (typeof meta.candidate === 'string') candidates.add(legacyName(meta.candidate))
        }
      })
      const orders = new Map(context.getClassOrder([...candidates]))
      function reorder(container) {
        const slots = []
        for (const [index, node] of (container.nodes ?? []).entries()) {
          if (node.nodes) reorder(node)
          const meta = node.raws.tailwind
          if (meta?.layer !== 'utilities' && meta?.parentLayer !== 'utilities') continue
          const order = typeof meta.candidate === 'string' ? orders.get(legacyName(meta.candidate)) : null
          if (order != null) slots.push({ index, node, order })
        }
        const sorted = [...slots].sort((a, b) => a.order < b.order ? -1 : a.order > b.order ? 1 : 0)
        slots.forEach(({ index }, position) => { container.nodes[index] = sorted[position].node })
      }
      reorder(root)
    },
  }
}
