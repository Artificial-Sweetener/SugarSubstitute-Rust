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
/** Keep private modules out of Comfy's recursive .js extension discovery. */
import { existsSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import ts from 'typescript';
import { walkArtifactFiles } from './WebArtifactSynchronization.js';

/** Rewrite module imports before moving the selected staged modules to ES module filenames. */
export function deferComfyModules(root: string, relativePaths: readonly string[]): void {
  if (!path.isAbsolute(root)) throw new Error('Artifact root must be absolute.');
  const files = walkArtifactFiles(root).filter((file) => file.endsWith('.js'));
  const available = new Set(files);
  const deferred = new Set<string>();
  for (const relative of relativePaths) {
    const file = path.resolve(root, relative);
    const within = path.relative(root, file);
    if (
      path.isAbsolute(relative) ||
      !within ||
      within === '..' ||
      within.startsWith(`..${path.sep}`) ||
      !file.endsWith('.js') ||
      !available.has(file) ||
      deferred.has(file) ||
      existsSync(`${file.slice(0, -3)}.mjs`)
    )
      throw new Error(`Invalid deferred module: ${relative}`);
    deferred.add(file);
  }
  const names = [...deferred].map((file) => path.basename(file));
  const changes: { file: string; source: string }[] = [];
  for (const file of files) {
    const source = readFileSync(file, 'utf8');
    if (!names.some((name) => source.includes(name))) continue;
    const parsed = ts.createSourceFile(
      file,
      source,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.JS,
    );
    const edits: { start: number; end: number; text: string }[] = [];
    const visit = (node: ts.Node): void => {
      let specifier: ts.Expression | undefined;
      let dynamic = false;
      if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) {
        specifier = node.moduleSpecifier;
      } else if (
        ts.isCallExpression(node) &&
        node.expression.kind === ts.SyntaxKind.ImportKeyword &&
        node.arguments.length === 1
      ) {
        specifier = node.arguments[0];
        dynamic = true;
      }
      if (specifier && ts.isStringLiteral(specifier) && specifier.text.startsWith('.')) {
        const target = path.resolve(path.dirname(file), specifier.text);
        if (deferred.has(target)) {
          if (!dynamic && !deferred.has(file))
            throw new Error(`Eager import of deferred module in ${path.relative(root, file)}`);
          edits.push({
            start: specifier.getStart(parsed) + 1,
            end: specifier.getEnd() - 1,
            text: `${specifier.text.slice(0, -3)}.mjs`,
          });
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(parsed);
    if (edits.length) {
      let updated = source;
      for (const edit of edits.sort((a, b) => b.start - a.start))
        updated = `${updated.slice(0, edit.start)}${edit.text}${updated.slice(edit.end)}`;
      changes.push({ file, source: updated });
    }
  }
  for (const change of changes) writeFileSync(change.file, change.source);
  for (const file of deferred) renameSync(file, `${file.slice(0, -3)}.mjs`);
}

/** Defer only browser-owned canvas presentation; shared workspace controls remain normal modules. */
export function deferBrowserCanvasModules(root: string): void {
  const base = 'comfyui/ui/output_canvas/';
  const named = [
    'infrastructure/ComfyBrowserCanvasHost.js',
    'infrastructure/ComfyCanvasModelRuntime.js',
    'application/CanvasCameraController.js',
    'application/CanvasCameraTransition.js',
    'application/CanvasImageAllowances.js',
    'application/CanvasImageBudget.js',
    'application/CanvasImageDecodeScheduler.js',
    'application/CanvasImagePreparation.js',
    'application/CanvasImageResources.js',
    'application/CanvasImageSlot.js',
    'domain/CanvasImageResource.js',
  ].map((name) => `${base}${name}`);
  const browser = walkArtifactFiles(path.join(root, base))
    .map((file) => path.relative(root, file).replaceAll('\\', '/'))
    .filter(
      (file) =>
        file.endsWith('.js') &&
        (file.startsWith(`${base}presentation/browser/BrowserCanvas`) ||
          (file.startsWith(`${base}infrastructure/BrowserCanvas`) &&
            !file.startsWith(`${base}infrastructure/BrowserCanvasWorkspace`))),
    );
  deferComfyModules(root, [...named, ...browser]);
}
