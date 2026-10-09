import fs from 'node:fs'
import postcss from 'postcss'
import valueParser from 'postcss-value-parser'
import cssesc from 'cssesc'

// Compare compiled declarations after resolving local/Tailwind/theme variables.
// Keep selector + variant checks as well as the requested global value sets.
function resolve(value, variables, seen = new Set()) {
  const parsed = valueParser(value)
  parsed.walk((node) => {
    if (node.type !== 'function' || node.value !== 'var') return
    const [name, ...fallback] = valueParser.stringify(node.nodes).split(',')
    const key = name.trim()
    if (seen.has(key)) throw new Error(`Circular CSS variable: ${key}`)
    const replacement = variables.get(key) ?? fallback.join(',').trim()
    if (replacement === undefined || replacement === '') return false
    node.type = 'word'
    node.value = resolve(replacement, variables, new Set([...seen, key]))
    delete node.nodes
    return false
  })
  return parsed.toString()
}

function canonical(value) {
  return value
    .replace(/'([^']*)'/g, '"$1"')
    .replace(/#([\da-f]{3,8})\b/gi, (match, hex) => {
      if (![3, 4, 6, 8].includes(hex.length)) return match
      if (hex.length < 5) hex = [...hex].map((digit) => digit + digit).join('')
      const channels = [0, 2, 4].map((offset) => parseInt(hex.slice(offset, offset + 2), 16))
      const alpha = hex.length === 8 ? parseInt(hex.slice(6), 16) / 255 : 1
      return `rgb(${channels.join(' ')} / ${alpha})`
    })
    .replace(/rgba?\(([^()]+)\)/g, (_, body) => {
      const parts = body.replace(/,/g, ' ').replace(/\//g, ' ').trim().split(/\s+/)
      if (parts.some((part) => !/^[\d.]+$/.test(part))) return `rgb(${body})`
      return `rgb(${parts.slice(0, 3).map(Number).join(' ')} / ${Number(parts[3] ?? 1)})`
    })
    .replace(/\s+/g, ' ')
    .replace(/\s*([(),])\s*/g, '$1')
    .trim()
}

function snapshot(file, replacements = {}) {
  const root = postcss.parse(fs.readFileSync(file, 'utf8'))
  const globals = new Map()
  root.walkRules((rule) => {
    if (rule.selector === ':root' || rule.selector.includes('::before')) {
      rule.walkDecls(/^--/, (decl) => globals.set(decl.prop, decl.value))
    }
  })
  const rules = new Map()
  const sets = Object.fromEntries(['colors', 'border-radius', 'box-shadow', 'font-size'].map((key) => [key, new Set()]))
  root.walkRules((rule) => {
    if (rule.selector === ':root') return
    const variables = new Map(globals)
    rule.walkDecls(/^--/, (decl) => variables.set(decl.prop, decl.value))
    let selector = rule.selector.replace(/\s+/g, ' ').trim()
    for (const [before, after] of Object.entries(replacements)) {
      selector = selector.replaceAll(cssesc(before, { isIdentifier: true }), cssesc(after, { isIdentifier: true }))
    }
    const context = []
    for (let parent = rule.parent; parent?.type !== 'root'; parent = parent?.parent) {
      if (parent?.type === 'atrule') context.unshift(`@${parent.name} ${parent.params}`)
    }
    const key = [...context, selector].join(' | ')
    const declarations = []
    rule.walkDecls((decl) => {
      if (decl.prop.startsWith('--') && !decl.prop.startsWith('--tw-')) return
      // Tailwind's optional shadow-color rewrite is unused in this application.
      // The applied --tw-shadow and resolved box-shadow are compared below.
      if (decl.prop === '--tw-shadow-colored') return
      const value = canonical(resolve(decl.value, variables))
      declarations.push(`${decl.prop}:${value}${decl.important ? '!important' : ''}`)
      if (sets[decl.prop]) sets[decl.prop].add(value)
      for (const match of value.matchAll(/rgb\([^()]+\)|\btransparent\b/g)) sets.colors.add(match[0])
    })
    rules.set(key, [...(rules.get(key) ?? []), ...declarations])
  })
  return {
    sets: Object.fromEntries(Object.entries(sets).map(([key, values]) => [key, [...values].sort()])),
    rules: Object.fromEntries(rules),
  }
}

const [command, baseline, current, mapFile] = process.argv.slice(2)
if (command === 'snapshot' && baseline && current) {
  const result = snapshot(baseline)
  fs.writeFileSync(current, JSON.stringify(result, null, 2) + '\n')
  console.log(Object.fromEntries(Object.entries(result.sets).map(([key, values]) => [key, values.length])))
} else if (command === 'compare' && baseline && current) {
  const mapping = mapFile ? JSON.parse(fs.readFileSync(mapFile, 'utf8')) : {}
  const before = snapshot(baseline, mapping)
  const after = snapshot(current)
  const failures = []
  for (const [key, values] of Object.entries(before.sets)) {
    const added = after.sets[key].filter((value) => !values.includes(value))
    const removed = values.filter((value) => !after.sets[key].includes(value))
    if (added.length || removed.length) failures.push({ set: key, added, removed })
  }
  for (const [selector, declarations] of Object.entries(before.rules)) {
    if (JSON.stringify(declarations) !== JSON.stringify(after.rules[selector])) {
      failures.push({ selector, before: declarations, after: after.rules[selector] })
    }
  }
  if (failures.length) {
    console.error(JSON.stringify(failures.slice(0, 30), null, 2))
    console.error(`${failures.length} CSS differences`)
    process.exitCode = 1
  } else {
    console.log(`Identical resolved value sets; ${Object.keys(before.rules).length} baseline rules preserved (including variants).`)
  }
} else {
  console.error('Usage: node scripts/design-css.mjs snapshot <css> <json> | compare <baseline.css> <current.css> [class-map.json]')
  process.exitCode = 1
}
