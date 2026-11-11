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
/** Preserve canonical browser owners when esbuild relocates imports into shared chunks. */
import path from 'node:path';
import { mkdirSync, writeFileSync } from 'node:fs';
import ts from 'typescript';
import type { OutputFile, Plugin } from 'esbuild';

/** Externalize exact source modules to their compiler-owned deployment counterparts. */
export function browserSharedModulePlugin(
  sourceRoot: string,
  runtimeRoot: string,
  modules: readonly string[],
): Plugin {
  const targets = new Map(
    modules.map((relative) => [path.join(sourceRoot, relative), path.join(runtimeRoot, relative)]),
  );
  return {
    name: 'shared-browser-owners',
    setup(builder) {
      builder.onResolve({ filter: /\.js$/ }, (args) => {
        const target = targets.get(path.resolve(args.resolveDir, args.path));
        return target ? { path: target, namespace: 'file', external: true } : undefined;
      });
    },
  };
}

/** Rebase import literals for each emitted location while preserving ordinary strings and module code. */
export function rebaseBrowserModuleImports(
  source: string,
  file: string,
  outputRoot: string,
  runtimeRoot: string,
  modules: readonly string[],
): string {
  const replacements = new Map(
    modules.map((relative) => {
      const target = path.join(runtimeRoot, relative);
      return [moduleUrl(outputRoot, target), moduleUrl(path.dirname(file), target)];
    }),
  );
  const parsed = ts.createSourceFile(file, source, ts.ScriptTarget.ES2022, true, ts.ScriptKind.JS);
  const edits: { start: number; end: number; text: string }[] = [];
  /** Rewrite only compiler-generated import/export specifiers whose authoritative target is known. */
  const visit = (node: ts.Node): void => {
    const specifier =
      ts.isImportDeclaration(node) || ts.isExportDeclaration(node)
        ? node.moduleSpecifier
        : ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword
          ? node.arguments[0]
          : undefined;
    if (specifier && ts.isStringLiteral(specifier)) {
      const target = replacements.get(specifier.text);
      if (target && target !== specifier.text)
        edits.push({
          start: specifier.getStart(parsed),
          end: specifier.getEnd(),
          text: JSON.stringify(target),
        });
    }
    ts.forEachChild(node, visit);
  };
  visit(parsed);
  for (const edit of edits.sort((a, b) => b.start - a.start))
    source = source.slice(0, edit.start) + edit.text + source.slice(edit.end);
  return source;
}

/** Publish a bundle without source maps after every shared import has its final relative address. */
export function publishBrowserBundle(
  files: readonly OutputFile[],
  outputRoot: string,
  runtimeRoot: string,
  modules: readonly string[],
): void {
  for (const file of files) {
    const relative = path.relative(outputRoot, file.path);
    if (relative.startsWith('..') || path.isAbsolute(relative))
      throw new Error('A browser bundle escaped its output directory.');
    if (file.path.endsWith('.map'))
      throw new Error('Shared browser imports require a bundle without source maps.');
    mkdirSync(path.dirname(file.path), { recursive: true });
    writeFileSync(
      file.path,
      file.path.endsWith('.js')
        ? rebaseBrowserModuleImports(file.text, file.path, outputRoot, runtimeRoot, modules)
        : file.contents,
    );
  }
}

/** Produce browser-relative module addresses from validated compiler output paths. */
function moduleUrl(from: string, target: string): string {
  const relative = path.relative(from, target).replaceAll('\\', '/');
  return relative.startsWith('.') ? relative : `./${relative}`;
}
