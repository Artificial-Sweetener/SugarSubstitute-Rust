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
/** Prove every deployed relative module reference resolves inside the served extension tree. */
import { existsSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import ts from 'typescript';
import { walkArtifactFiles } from './WebArtifactSynchronization.js';

/**
 * Reject relative imports that leave the root or name a missing module.
 *
 * Comfy eagerly imports every deployed `.js` extension module, so one unresolved reference
 * fails the whole extension in every host. All failures are reported together.
 */
export function assertRelativeModulesResolve(root: string): void {
  if (!path.isAbsolute(root)) throw new Error('Artifact root must be absolute.');
  const unresolved: string[] = [];
  const modules = walkArtifactFiles(root).filter(
    (file) => file.endsWith('.js') || file.endsWith('.mjs'),
  );
  for (const file of modules) {
    const parsed = ts.createSourceFile(
      file,
      readFileSync(file, 'utf8'),
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.JS,
    );
    const visit = (node: ts.Node): void => {
      let specifier: ts.Expression | undefined;
      if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) {
        specifier = node.moduleSpecifier;
      } else if (
        ts.isCallExpression(node) &&
        node.expression.kind === ts.SyntaxKind.ImportKeyword &&
        node.arguments.length === 1
      ) {
        specifier = node.arguments[0];
      }
      if (specifier && ts.isStringLiteralLike(specifier) && specifier.text.startsWith('.')) {
        const target = path.resolve(path.dirname(file), specifier.text);
        const within = path.relative(root, target);
        const escapes = within === '..' || within.startsWith(`..${path.sep}`);
        if (escapes || !existsSync(target) || !statSync(target).isFile())
          unresolved.push(`${path.relative(root, file)} -> ${specifier.text}`);
      }
      ts.forEachChild(node, visit);
    };
    visit(parsed);
  }
  if (unresolved.length > 0)
    throw new Error(`Unresolved deployed module references:\n${unresolved.join('\n')}`);
}
