import fs from 'node:fs'
import path from 'node:path'
import ts from 'typescript'
import { colorTokens, utilityTokens } from './design-token-map.mjs'

const escape = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
const utilities = Object.keys(utilityTokens).sort((a, b) => b.length - a.length)
const colorUtility =
  '(?:bg|text|border(?:-[trblxyse])?|ring(?:-offset)?|accent|from|via|to|divide|placeholder|decoration|fill|stroke|outline)'
const opacity = '(?:/(?:\\[[\\d.]+\\]|[\\d.]+))?'
const pattern = new RegExp(
  `(?<![\\w-])(?:${utilities.map(escape).join('|')}|${colorUtility}-(?:${Object.keys(colorTokens).map(escape).join('|')})${opacity})(?![\\w-])`,
  'g',
)
const originalColors = Object.fromEntries(
  Object.entries(colorTokens).map(([original, token]) => [token, original]),
)
const tokenPattern = new RegExp(
  `(?<![\\w-])(${colorUtility})-(${Object.keys(originalColors)
    .sort((a, b) => b.length - a.length)
    .map(escape)
    .join('|')})(${opacity})(?![\\w-])`,
  'g',
)

export function migrateDesignClasses(source, mapping = {}) {
  return source.replace(pattern, (before) => {
    const color = Object.keys(colorTokens).find((name) =>
      before.includes(`-${name}`),
    )
    const after =
      utilityTokens[before] ??
      before.replace(`-${color}`, `-${colorTokens[color]}`)
    mapping[before] = after
    return after
  })
}

function migrateFile(source, file, mapping) {
  if (file.endsWith('.css')) return migrateDesignClasses(source, mapping)
  const tree = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true)
  const edits = []
  function visit(node) {
    if (ts.isStringLiteral(node) || ts.isTemplateLiteralToken(node)) {
      const start = node.getStart(tree)
      const before = source.slice(start, node.end)
      const after = migrateDesignClasses(before, mapping)
      if (before !== after) edits.push({ start, end: node.end, after })
      return
    }
    ts.forEachChild(node, visit)
  }
  visit(tree)
  return edits
    .sort((a, b) => b.start - a.start)
    .reduce(
      (text, { start, end, after }) =>
        text.slice(0, start) + after + text.slice(end),
      source,
    )
}

if (process.argv[1] === import.meta.filename) {
  const args = process.argv.slice(2)
  const check = args.includes('--check')
  const mapIndex = args.indexOf('--map')
  const mapping = { ...utilityTokens }
  // Include all prefix/opacity forms actually present; selectors keep variants.
  let changed = 0
  for (const relative of fs.readdirSync('src', { recursive: true }).sort()) {
    if (!/\.(?:tsx?|jsx?|css)$/.test(relative)) continue
    const file = path.join('src', relative)
    const before = fs.readFileSync(file, 'utf8')
    const after = migrateFile(before, file, mapping)
    // Re-running --map after migration must reproduce the same evidence rather
    // than losing the color/opacity entries because no raw classes remain.
    for (const match of after.matchAll(tokenPattern)) {
      mapping[`${match[1]}-${originalColors[match[2]]}${match[3]}`] = match[0]
    }
    if (migrateFile(after, file, {}) !== after)
      throw new Error(`Non-idempotent migration: ${file}`)
    if (before === after) continue
    changed++
    if (check) console.error(file)
    else fs.writeFileSync(file, after)
  }
  if (mapIndex >= 0)
    fs.writeFileSync(
      args[mapIndex + 1],
      JSON.stringify(mapping, null, 2) + '\n',
    )
  process.stdout.write(
    `${changed} files ${check ? 'require migration' : 'migrated'}\n`,
  )
  if (check && changed) process.exitCode = 1
}
