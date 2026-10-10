import fs from 'node:fs'
import path from 'node:path'
import ts from 'typescript'

// Presentation wrappers use separators, not outlines. Control surfaces and the
// log well are deliberately excluded; JSX parsing avoids matching class prose.
export function checkDesignRows(files) {
  let violations = 0
  for (const file of files.filter((name) => name.endsWith('.tsx'))) {
    const source = ts.createSourceFile(
      file,
      fs.readFileSync(file, 'utf8'),
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TSX,
    )
    const icons = new Set(['svg'])
    for (const statement of source.statements) {
      if (
        ts.isImportDeclaration(statement) &&
        statement.moduleSpecifier.text === 'lucide-react' &&
        statement.importClause?.namedBindings &&
        ts.isNamedImports(statement.importClause.namedBindings)
      )
        for (const element of statement.importClause.namedBindings.elements)
          icons.add(element.name.text)
    }
    const visit = (node) => {
      if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
        const tag = node.tagName.getText(source)
        if (tag === 'Panel') {
          const leading = node.attributes.properties.find(
            (attribute) =>
              ts.isJsxAttribute(attribute) &&
              attribute.name.getText(source) === 'leading',
          )
          const inspectLeading = (part, functional = false) => {
            const opening = ts.isJsxElement(part)
              ? part.openingElement
              : ts.isJsxSelfClosingElement(part)
                ? part
                : null
            if (opening) {
              const attributes = opening.attributes.properties.filter(
                ts.isJsxAttribute,
              )
              const named = attributes.some((attribute) =>
                ['aria-label', 'aria-labelledby', 'label'].includes(
                  attribute.name.getText(source),
                ),
              )
              const control = ['button', 'Button', 'IconButton'].includes(
                opening.tagName.getText(source),
              )
              const text =
                ts.isJsxElement(part) &&
                part.children.some(
                  (child) => ts.isJsxText(child) && child.text.trim(),
                )
              functional ||= named || (control && text)
              if (icons.has(opening.tagName.getText(source)) && !functional) {
                const { line } = source.getLineAndCharacterOfPosition(
                  opening.getStart(source),
                )
                console.error(
                  `${file}:${line + 1}: decorative icon in Panel leading — keep only named functional controls or status indicators`,
                )
                violations++
              }
            }
            ts.forEachChild(part, (child) => inspectLeading(child, functional))
          }
          if (leading?.initializer) inspectLeading(leading.initializer)
        }
        if (['input', 'select'].includes(tag)) {
          const { line } = source.getLineAndCharacterOfPosition(
            node.getStart(source),
          )
          console.error(
            `${file}:${line + 1}: native ${tag} in feature — use Input or DarkSelect`,
          )
          violations++
        }
        if (['div', 'label', 'p'].includes(tag)) {
          const attributes = node.attributes.properties.filter(
            ts.isJsxAttribute,
          )
          const role = attributes.find(
            (attribute) => attribute.name.getText(source) === 'role',
          )?.initializer
          const control =
            role &&
            ts.isStringLiteral(role) &&
            [
              'button',
              'listbox',
              'slider',
              'switch',
              'checkbox',
              'radio',
              'combobox',
              'textbox',
              'dialog',
            ].includes(role.text)
          const attribute = attributes.find(
            (item) => item.name.getText(source) === 'className',
          )
          const strings = []
          const collect = (part) => {
            if (
              ts.isStringLiteral(part) ||
              ts.isNoSubstitutionTemplateLiteral(part) ||
              ts.isTemplateHead(part) ||
              ts.isTemplateMiddle(part) ||
              ts.isTemplateTail(part)
            )
              strings.push(part.text)
            ts.forEachChild(part, collect)
          }
          if (attribute?.initializer) collect(attribute.initializer)
          const classes = strings.join(' ')
          const logWell =
            path.basename(file) === 'LogPanelView.tsx' &&
            classes.includes('font-mono') &&
            classes.includes('overflow-y-auto')
          if (!control && !logWell && /(?:^|\s)border(?:\s|$)/.test(classes)) {
            const { line } = source.getLineAndCharacterOfPosition(
              node.getStart(source),
            )
            console.error(
              `${file}:${line + 1}: outlined presentation wrapper — use border-t border-line`,
            )
            violations++
          }
        }
      }
      ts.forEachChild(node, visit)
    }
    visit(source)
  }
  return violations
}
