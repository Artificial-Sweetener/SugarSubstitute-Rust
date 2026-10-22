//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Keep private native editor modules and bare npm imports out of Comfy's eagerly loaded extension tree. */
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import ts from 'typescript';

/** Inventory deployed JavaScript rather than assuming a particular bundler chunk name. */
function browserModules(root: string): string[] {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(root, entry.name);
    return entry.isDirectory() ? browserModules(file) : entry.name.endsWith('.js') ? [file] : [];
  });
}

test('deploys the native editor through browser bundles with resolvable module specifiers', () => {
  expect(existsSync('web/sugarprompt') ? browserModules('web/sugarprompt') : []).toEqual([]);
  const bareImports: string[] = [];
  for (const file of browserModules('web')) {
    const source = ts.createSourceFile(
      file,
      readFileSync(file, 'utf8'),
      ts.ScriptTarget.ES2022,
      true,
      ts.ScriptKind.JS,
    );
    const visit = (node: ts.Node): void => {
      const specifier =
        ts.isImportDeclaration(node) || ts.isExportDeclaration(node) ? node.moduleSpecifier : null;
      if (specifier && ts.isStringLiteral(specifier) && !/^(?:\.|\/|https?:)/u.test(specifier.text))
        bareImports.push(`${file}: ${specifier.text}`);
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  expect(bareImports).toEqual([]);
});
