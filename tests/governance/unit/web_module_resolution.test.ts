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
/** Deployed module graphs must resolve inside the served extension tree. */
import { afterEach, beforeEach } from '@jest/globals';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { assertDeployedModulesResolve } from '../../../scripts/WebModuleResolution.js';

let root: string;
beforeEach(() => {
  const parent = mkdtempSync(
    path.join(path.resolve('D:/devartifacts/SugarCubes/output-saving'), 'module-resolution-'),
  );
  root = path.join(parent, 'web');
  mkdirSync(path.join(root, 'ui/infrastructure'), { recursive: true });
});
afterEach(() => rmSync(path.dirname(root), { recursive: true, force: true }));

test('accepts static, re-exported, dynamic and deferred .mjs references that resolve', () => {
  writeFileSync(path.join(root, 'ui/shared.js'), 'export const value = 1;\n');
  writeFileSync(path.join(root, 'ui/lazy.mjs'), 'export const lazy = 2;\n');
  writeFileSync(
    path.join(root, 'ui/infrastructure/main.js'),
    "import { value } from '../shared.js';\nexport { value } from '../shared.js';\n" +
      "export const load = () => import('../lazy.mjs');\nconst note = '../absent.js';\n" +
      "import '/scripts/app.js';\n",
  );
  expect(() => assertDeployedModulesResolve(root)).not.toThrow();
});

test('rejects a reference that escapes the served tree even when the target exists', () => {
  writeFileSync(path.join(path.dirname(root), 'outside.js'), 'export const value = 1;\n');
  writeFileSync(
    path.join(root, 'ui/infrastructure/preview.js'),
    "import { value } from '../../../outside.js';\n",
  );
  expect(() => assertDeployedModulesResolve(root)).toThrow(
    /preview\.js -> \.\.\/\.\.\/\.\.\/outside\.js/,
  );
});

test('reports every missing static and dynamic module together', () => {
  writeFileSync(
    path.join(root, 'ui/infrastructure/one.js'),
    "import { a } from './missing-a.js';\nexport const load = () => import('./missing-b.js');\n",
  );
  writeFileSync(path.join(root, 'ui/two.js'), "export * from './infrastructure';\n");
  let message = '';
  try {
    assertDeployedModulesResolve(root);
  } catch (error: unknown) {
    message = error instanceof Error ? error.message : String(error);
  }
  expect(message).toContain('missing-a.js');
  expect(message).toContain('missing-b.js');
  expect(message).toContain('two.js -> ./infrastructure');
});

test('rejects bare package specifiers that a browser cannot resolve without an import map', () => {
  writeFileSync(
    path.join(root, 'ui/infrastructure/overlay.js'),
    "import { h } from 'vue';\nexport { ref } from 'vue';\n" +
      "export const load = () => import('@scope/editor');\nconst note = 'vue';\n",
  );
  let message = '';
  try {
    assertDeployedModulesResolve(root);
  } catch (error: unknown) {
    message = error instanceof Error ? error.message : String(error);
  }
  expect(message).toContain('overlay.js -> vue');
  expect(message).toContain('overlay.js -> @scope/editor');
  expect(message.match(/overlay\.js -> vue/g)).toHaveLength(2);
});

test('the committed deployment tree loads without unresolved or bare module references', () => {
  expect(() => assertDeployedModulesResolve(path.resolve('web'))).not.toThrow();
});

test('rejects a relative artifact root', () => {
  expect(() => assertDeployedModulesResolve('web')).toThrow(/absolute/);
});
