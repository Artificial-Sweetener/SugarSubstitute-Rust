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
/** Preserve lazy modules while Comfy enumerates every deployed .js extension. */
import { afterEach, beforeEach } from '@jest/globals';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { deferComfyModules } from '../../../scripts/DeferredComfyModules.js';

let root: string;
beforeEach(() => {
  root = mkdtempSync(path.join(tmpdir(), 'lazy-modules-'));
  mkdirSync(path.join(root, 'private'));
});
afterEach(() => rmSync(root, { recursive: true, force: true }));

test('rewrites only module references and leaves private files out of the .js extension inventory', () => {
  writeFileSync(
    path.join(root, 'main.js'),
    "const note = './private/one.js';\nexport const load = () => import('./private/one.js');\n",
  );
  writeFileSync(path.join(root, 'private/one.js'), "export { value } from './two.js';\n");
  writeFileSync(path.join(root, 'private/two.js'), 'export const value = 42;\n');
  deferComfyModules(root, ['private/one.js', 'private/two.js']);
  expect(readFileSync(path.join(root, 'main.js'), 'utf8')).toContain("import('./private/one.mjs')");
  expect(readFileSync(path.join(root, 'main.js'), 'utf8')).toContain(
    "const note = './private/one.js'",
  );
  expect(readFileSync(path.join(root, 'private/one.mjs'), 'utf8')).toContain("from './two.mjs'");
  expect(existsSync(path.join(root, 'private/one.js'))).toBe(false);
  expect(existsSync(path.join(root, 'private/two.js'))).toBe(false);
});

test('rejects an eager dependency before changing any staged file', () => {
  writeFileSync(path.join(root, 'main.js'), "import './private/one.js';\n");
  writeFileSync(path.join(root, 'private/one.js'), 'export const value = 1;\n');
  expect(() => deferComfyModules(root, ['private/one.js'])).toThrow(/eager/i);
  expect(readFileSync(path.join(root, 'main.js'), 'utf8')).toContain("'./private/one.js'");
  expect(existsSync(path.join(root, 'private/one.js'))).toBe(true);
  expect(existsSync(path.join(root, 'private/one.mjs'))).toBe(false);
});

test('rejects paths outside the staged tree and missing modules', () => {
  expect(() => deferComfyModules(root, ['../outside.js'])).toThrow();
  expect(() => deferComfyModules(root, ['private/missing.js'])).toThrow();
});
