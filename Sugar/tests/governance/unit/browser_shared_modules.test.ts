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
/** Prove shared-owner imports retain their deployment address after bundle chunk relocation. */
import path from 'node:path';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import type { OutputFile } from 'esbuild';
import {
  publishBrowserBundle,
  rebaseBrowserModuleImports,
} from '../../../scripts/BrowserSharedModules.js';

/** Model the compiler-owned artifact boundary without weakening source types. */
function artifact(file: string, text: string): OutputFile {
  return { path: file, text, contents: Buffer.from(text), hash: 'fixture' };
}

test('rebases declarations and dynamic imports without changing strings or module behavior', () => {
  const root = path.resolve('runtime/comfyui/ui');
  const file = path.join(root, 'output_canvas/presentation/vue/CanvasOverlayRuntime.js');
  const source = `import {ContextMenu as Menu} from "./menus/ContextMenu.js";
export {FloatingSurface} from "./menus/FloatingSurface.js";
const value = import("./menus/FloatingSurface.js");
const literal = 'from "./menus/ContextMenu.js"';`;
  const result = rebaseBrowserModuleImports(source, file, root, root, [
    'menus/ContextMenu.js',
    'menus/FloatingSurface.js',
  ]);
  expect(result).toContain('from "../../../menus/ContextMenu.js"');
  expect(result).toContain('from "../../../menus/FloatingSurface.js"');
  expect(result).toContain('import("../../../menus/FloatingSurface.js")');
  expect(result).toContain(`const literal = 'from "./menus/ContextMenu.js"';`);
});

test('retains chunk-level addresses and ignores unrelated imports', () => {
  const root = path.resolve('runtime/comfyui/ui');
  const source = 'import {FloatingSurface} from "./menus/FloatingSurface.js";import "./chunk.js";';
  expect(
    rebaseBrowserModuleImports(source, path.join(root, 'chunk.js'), root, root, [
      'menus/FloatingSurface.js',
    ]),
  ).toBe(source);
});

test('publishes rebased modules and preserves binary compiler artifacts', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'sc-shared-bundle-'));
  try {
    publishBrowserBundle(
      [
        artifact(path.join(root, 'nested/entry.js'), 'import "./menus/FloatingSurface.js";'),
        artifact(path.join(root, 'asset.bin'), 'bytes'),
      ],
      root,
      root,
      ['menus/FloatingSurface.js'],
    );
    expect(readFileSync(path.join(root, 'nested/entry.js'), 'utf8')).toBe(
      'import "../menus/FloatingSurface.js";',
    );
    expect(readFileSync(path.join(root, 'asset.bin'), 'utf8')).toBe('bytes');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects out-of-directory artifacts and unsupported source maps before publication', () => {
  const root = path.resolve('runtime/comfyui/ui');
  expect(() =>
    publishBrowserBundle([artifact(path.resolve(root, '../escape.js'), '')], root, root, []),
  ).toThrow('escaped');
  expect(() =>
    publishBrowserBundle([artifact(path.join(root, 'entry.js.map'), '')], root, root, []),
  ).toThrow('without source maps');
});
