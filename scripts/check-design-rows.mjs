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
    const visit = (node) => {
      if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
        const tag = node.tagName.getText(source)
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
