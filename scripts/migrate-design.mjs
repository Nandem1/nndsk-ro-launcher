import fs from 'node:fs'
import path from 'node:path'
import { colorTokens, utilityTokens } from './design-token-map.mjs'

const escape = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
const utilities = Object.keys(utilityTokens).sort((a, b) => b.length - a.length)
const pattern = new RegExp(
  `(?<![\\w-])(?:${utilities.map(escape).join('|')}|(?:bg|text|border(?:-[trblxyse])?|ring(?:-offset)?|accent|from|via|to|divide|placeholder|decoration|fill|stroke|outline)-(?:${Object.keys(colorTokens).map(escape).join('|')})(?:/(?:\\[[\\d.]+\\]|[\\d.]+))?)(?![\\w-])`,
  'g',
)

export function migrateDesignClasses(source, mapping = {}) {
  return source.replace(pattern, (before) => {
    const color = Object.keys(colorTokens).find((name) => before.includes(`-${name}`))
    const after = utilityTokens[before] ?? before.replace(`-${color}`, `-${colorTokens[color]}`)
    mapping[before] = after
    return after
  })
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
    const after = migrateDesignClasses(before, mapping)
    if (migrateDesignClasses(after) !== after) throw new Error(`Non-idempotent migration: ${file}`)
    if (before === after) continue
    changed++
    if (check) console.error(file)
    else fs.writeFileSync(file, after)
  }
  if (mapIndex >= 0) fs.writeFileSync(args[mapIndex + 1], JSON.stringify(mapping, null, 2) + '\n')
  console.log(`${changed} files ${check ? 'require migration' : 'migrated'}`)
  if (check && changed) process.exitCode = 1
}
