// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Read actual upstream type contracts without importing their unrelated desktop implementation. */
import ts from 'typescript'

export function declarations(source, names) {
  const tree = ts.createSourceFile('upstream.ts', source, ts.ScriptTarget.Latest, true)
  if (tree.parseDiagnostics.length) throw new Error('Upstream type source does not parse')
  const selected = tree.statements.filter(node =>
    (ts.isInterfaceDeclaration(node) || ts.isTypeAliasDeclaration(node)) && names.includes(node.name.text))
  if (selected.length !== names.length || new Set(selected.map(node => node.name.text)).size !== names.length) throw new Error('Upstream type contract changed')
  const references = new Set()
  function visit(node) {
    if (ts.isIdentifier(node)) references.add(node.text)
    ts.forEachChild(node, visit)
  }
  for (let index = 0; index < selected.length; index++) {
    visit(selected[index])
    for (const node of tree.statements) {
      if ((ts.isInterfaceDeclaration(node) || ts.isTypeAliasDeclaration(node)) && references.has(node.name.text) && !selected.includes(node)) selected.push(node)
    }
  }
  const imports = tree.statements.filter(node => {
    if (!ts.isImportDeclaration(node) || !node.importClause?.isTypeOnly) return false
    const clause = node.importClause
    if (clause.name && references.has(clause.name.text)) return true
    const bindings = clause.namedBindings
    return bindings && (ts.isNamespaceImport(bindings) ? references.has(bindings.name.text) : bindings.elements.some(item => references.has(item.name.text)))
  })
  return [...imports, ...selected].map(node => node.getFullText(tree)).join('\n') + '\n'
}
