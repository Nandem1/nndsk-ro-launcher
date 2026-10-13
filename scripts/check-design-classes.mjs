import fs from 'node:fs'
import path from 'node:path'
import ts from 'typescript'
import postcss from 'postcss'
import selectorParser from 'postcss-selector-parser'
import { compile } from '@tailwindcss/node'

const root = path.resolve(import.meta.dirname, '..')
const utility =
  /^(?:(?:bg|text|border|ring|rounded|shadow|fill|stroke|divide|outline|placeholder|from|to|via)-|(?:border|ring|rounded|shadow|outline)$)/

// Split variants outside arbitrary-value brackets (e.g. hover:bg-[rgb(...)])
// without losing the exact candidate that Tailwind must emit.
const base = (value) => {
  let depth = 0
  let start = 0
  for (let i = 0; i < value.length; i++) {
    if ('[('.includes(value[i])) depth++
    if ('])'.includes(value[i])) depth--
    if (value[i] === ':' && depth === 0) start = i + 1
  }
  return value.slice(start).replace(/^!/, '')
}
const words = (text) => text.split(/\s+/).filter(Boolean)
const isLiteral = (node) =>
  ts.isStringLiteral(node) ||
  ts.isNoSubstitutionTemplateLiteral(node) ||
  ts.isTemplateHead(node) ||
  ts.isTemplateMiddle(node) ||
  ts.isTemplateTail(node)

export async function checkDesignClasses(files) {
  const inputs = files.filter((file) => /\.[jt]sx?(?:\.fixture)?$/.test(file))
  const sources = inputs.map((file) => ({
    file,
    text: fs.readFileSync(file, 'utf8'),
  }))
  // Compile the real stylesheet with Tailwind 4 for every word found in the checked sources'
  // string literals. Only words that are valid utilities produce CSS, so this superset is safe.
  const stylesheet = path.join(root, 'src/index.css')
  const compiler = await compile(fs.readFileSync(stylesheet, 'utf8'), {
    base: path.dirname(stylesheet),
    onDependency: () => {},
  })
  const words_ = new Set()
  for (const { file, text } of sources) {
    const source = ts.createSourceFile(
      file,
      text,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TSX,
    )
    const collect = (node) => {
      if (isLiteral(node)) words(node.text).forEach((word) => words_.add(word))
      ts.forEachChild(node, collect)
    }
    collect(source)
  }
  const result = { root: postcss.parse(compiler.build([...words_])) }
  const emitted = new Set()
  const borderColors = new Set()
  result.root.walkRules((rule) => {
    const names = []
    selectorParser((selectors) =>
      selectors.walkClasses((node) => names.push(node.value)),
    ).processSync(rule.selector)
    names.forEach((name) => emitted.add(name))
    if (
      rule.nodes.some(
        (node) =>
          node.type === 'decl' &&
          (/^(?:border(?:-[a-z]+)?-color|border-color)$/.test(node.prop) ||
            (node.prop === 'border' &&
              /(?:rgb|var\(--c-|transparent|currentColor)/.test(node.value))),
      )
    )
      names.forEach((name) => borderColors.add(name))
  })
  let violations = 0
  let candidates = 0
  for (const { file, text } of sources) {
    const source = ts.createSourceFile(
      file,
      text,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TSX,
    )
    const declarations = new Map()
    const index = (node) => {
      if (
        ts.isVariableDeclaration(node) &&
        ts.isIdentifier(node.name) &&
        node.initializer
      )
        declarations.set(node.name.text, node.initializer)
      ts.forEachChild(node, index)
    }
    index(source)
    const strings = (node, seen = new Set()) => {
      if (!node) return []
      if (isLiteral(node)) return [node.text]
      if (
        ts.isIdentifier(node) &&
        declarations.has(node.text) &&
        !seen.has(node.text)
      )
        return strings(
          declarations.get(node.text),
          new Set([...seen, node.text]),
        )
      const values = []
      ts.forEachChild(node, (child) => {
        values.push(...strings(child, seen))
      })
      return values
    }
    const fail = (node, message) => {
      const { line } = source.getLineAndCharacterOfPosition(
        node.getStart(source),
      )
      console.error(`${path.relative(root, file)}:${line + 1}: ${message}`)
      violations++
    }
    const seenCandidates = new Set()
    const visit = (node) => {
      if (isLiteral(node)) {
        for (const candidate of words(node.text)) {
          if (!utility.test(base(candidate)) || seenCandidates.has(candidate))
            continue
          seenCandidates.add(candidate)
          candidates++
          if (!emitted.has(candidate))
            fail(
              node,
              `unknown design class ${candidate} — no compiled CSS rule`,
            )
        }
      }
      if (
        ts.isJsxAttribute(node) &&
        node.name.getText(source) === 'className'
      ) {
        const classes = strings(node.initializer).flatMap(words)
        const hasWidth = classes.some((word) =>
          /^(?:border(?:-[tblrxyse])?(?:-(?:[1248]|\[[^\]]+\]))?|divide-[xy](?:-[1248])?)$/.test(
            base(word),
          ),
        )
        const hasColor = classes.some((word) => borderColors.has(word))
        if (hasWidth && !hasColor)
          fail(
            node,
            'border/divider width without explicit border color or CSS primitive color',
          )
      }
      ts.forEachChild(node, visit)
    }
    visit(source)
  }
  process.stdout.write(
    `Compiled design classes: ${candidates} candidates, ${violations} violations\n`,
  )
  return violations
}
