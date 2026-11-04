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
/** Prove every deployed module and asset reference resolves inside the served extension tree. */
import { existsSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import ts from 'typescript';
import { walkArtifactFiles } from './WebArtifactSynchronization.js';

/**
 * Reject relative imports that leave the root or name a missing module, bare specifiers, and
 * `new URL(path, import.meta.url)` assets that leave the root or name nothing.
 *
 * Comfy eagerly imports every deployed `.js` extension module, so one unresolved reference
 * fails the whole extension in every host. Comfy serves no import map, so a bare package
 * specifier such as `vue` never resolves; package code must be bundled. Absolute paths name
 * host-served modules such as `/scripts/app.js`. An asset URL resolves against the module
 * that finally holds it, so bundling it into a chunk elsewhere moves its target; a template
 * path is checked up to its first substitution, which must name an existing directory or file.
 * All failures are reported together.
 */
export function assertDeployedModulesResolve(root: string): void {
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
      if (specifier && ts.isStringLiteralLike(specifier) && !specifier.text.startsWith('/')) {
        if (!resolvesWithin(root, file, specifier.text))
          unresolved.push(`${path.relative(root, file)} -> ${specifier.text}`);
      }
      const asset = moduleAsset(node);
      if (asset !== null && !assetResolvesWithin(root, file, asset))
        unresolved.push(`${path.relative(root, file)} -> ${asset}`);
      ts.forEachChild(node, visit);
    };
    visit(parsed);
  }
  if (unresolved.length > 0)
    throw new Error(`Unresolved deployed module references:\n${unresolved.join('\n')}`);
}

/** Resolve one relative specifier to an existing file inside the served tree. */
function resolvesWithin(root: string, file: string, specifier: string): boolean {
  if (!specifier.startsWith('.')) return false;
  const target = path.resolve(path.dirname(file), specifier);
  const within = path.relative(root, target);
  const escapes = within === '..' || within.startsWith(`..${path.sep}`);
  return !escapes && existsSync(target) && statSync(target).isFile();
}

/** The relative path of `new URL(path, import.meta.url)`, up to a template's first substitution. */
function moduleAsset(node: ts.Node): string | null {
  if (!ts.isNewExpression(node) || node.arguments?.length !== 2) return null;
  const [target, base] = node.arguments;
  const relativeToModule =
    base !== undefined &&
    ts.isPropertyAccessExpression(base) &&
    ts.isMetaProperty(base.expression) &&
    base.expression.keywordToken === ts.SyntaxKind.ImportKeyword &&
    base.name.text === 'url';
  if (!relativeToModule || !ts.isIdentifier(node.expression) || node.expression.text !== 'URL')
    return null;
  if (target !== undefined && ts.isStringLiteralLike(target)) return target.text;
  if (target !== undefined && ts.isTemplateExpression(target)) return target.head.text;
  return null;
}

/** Resolve an asset path, or a template's static prefix, to something inside the served tree. */
function assetResolvesWithin(root: string, file: string, asset: string): boolean {
  if (!asset.startsWith('.')) return false;
  const target = path.resolve(path.dirname(file), asset);
  const within = path.relative(root, target);
  const escapes = within === '..' || within.startsWith(`..${path.sep}`);
  if (escapes) return false;
  if (asset.endsWith('/')) return existsSync(target) && statSync(target).isDirectory();
  return existsSync(target) && statSync(target).isFile();
}
