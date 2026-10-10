import fs from 'node:fs'
import path from 'node:path'
import { checkDesignRows } from './check-design-rows.mjs'

const root = path.resolve(import.meta.dirname, '..')
const catalog = path.join(root, 'scripts/design-token-map.mjs')
const sourceExtensions = /\.(?:[cm]?[jt]sx?|css|html)$/
const forbidden = [
  // Match inside hover/focus/group/important variants and opacity modifiers.
  /\b(?:slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\d+\b/g,
  /\b(?:bg|text|border(?:-[trblxyse])?|ring(?:-offset)?|from|via|to|divide|placeholder|decoration|fill|stroke|outline|accent)-(?:white|black)\b/g,
  /\btext-\[(?:\d+(?:\.\d+)?|\.\d+)px\]/gi,
  /(?<![\w-])rounded-(?:none|sm|md|lg|xl|2xl|3xl|full)(?![\w-])/g,
  /\brounded-\[[^\]]+\]/g,
  /(?<![\w-])shadow-(?:sm|md|lg|xl|2xl|inner|glass|glow-(?:amber|emerald|red)|\[[^\]]+\])(?![\w-])/g,
  /\b(?:bg-(?:panel|progress)-gradient|bg-gradient-[\w-]+|backdrop\x2dblur(?:-[\w-]+)?|shadow-(?:glow|dot)-[\w-]+|overlay\x2dlight|tracking-\[[^\]]+\])(?![\w-])/g,
  /\b(?:ink-(?:soft|dim|bright)|accent-(?:ink|soft|light|bright|strong)|ok-(?:ink|soft|bright)|bad-(?:ink|soft|bright|strong)|info-(?:soft|bright|strong)|special-(?:ink|soft))(?![\w-])/g,
  /\b(?:stagger\x2dchildren|animate-(?:fade-rise|scale-in|rail-expand|rail-collapse|stat-flash-red|stat-flash-blue)|transition-(?:all|transform|opacity)|duration-(?:200|300|400|500))\b/g,
  /\baccent\x2daccent\b/g,
  /\bopacity\x2d\d+\b/g,
  /\bfont\x2dwordmark\b/g,
  /\buppercase\s+tracking-(?:wide|wider|widest)\b/g,
  /\bborder-t-(?:ok|warn|bad|info)\b/g,
]

function files(target) {
  const stat = fs.lstatSync(target)
  if (stat.isSymbolicLink()) return []
  if (stat.isDirectory())
    return fs
      .readdirSync(target)
      .flatMap((name) => files(path.join(target, name)))
  return sourceExtensions.test(target) && target !== catalog ? [target] : []
}

const targets = process.argv.slice(2)
const paths = targets.length
  ? targets.map((target) => path.resolve(target))
  : [
      'src',
      'scripts',
      'index.html',
      'tailwind.config.js',
      'postcss.config.js',
    ].map((target) => path.join(root, target))
let violations = 0
let checked = 0
const sources = paths.flatMap(files)
for (const file of sources) {
  checked++
  const source = fs.readFileSync(file, 'utf8')
  for (const pattern of forbidden) {
    for (const match of source.matchAll(pattern)) {
      const line = source.slice(0, match.index).split('\n').length
      console.error(
        `${path.relative(root, file)}:${line}: ${match[0]} — use a design token or primitive`,
      )
      violations++
    }
  }
}
violations += checkDesignRows(
  sources.filter(
    (file) =>
      targets.length ||
      ['src/features/', 'src/app/'].some((directory) =>
        file.startsWith(path.join(root, directory)),
      ),
  ),
)
if (violations) {
  console.error(`Design guard failed: ${violations} design violations`)
  process.exitCode = 1
} else {
  console.log(
    `Design guard passed: ${checked} files, zero raw design references`,
  )
  await import('./check-design-contrast.mjs')
}
