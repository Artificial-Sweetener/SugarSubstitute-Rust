/** @jest-environment node */
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
/** Prove UXP preparation builds and validates fixture packages without host installation. */
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { buildPhotoshop } from '../../../scripts/photoshop/build.js';
import {
  packagePath,
  record,
  validateManifest,
  validatePackage,
} from '../../../scripts/photoshop/manifest.js';

test('the SugarCubes panel declares room for independent width and height resizing', () => {
  const authored: unknown = JSON.parse(
    readFileSync('integrations/photoshop/manifest.json', 'utf8'),
  );
  const manifest = record(authored, 'manifest');
  if (!Array.isArray(manifest.entrypoints)) throw new Error('Missing panel entrypoints.');
  const panel = record(manifest.entrypoints[0], 'panel');
  const maximum = record(panel.maximumSize, 'maximumSize');
  expect(maximum.width).toBeGreaterThanOrEqual(3840);
  expect(maximum.height).toBeGreaterThanOrEqual(2160);
});

/** Return a minimal independent fixture identity, host and explicit WebView permission. */
function manifest(): Record<string, unknown> {
  return {
    manifestVersion: 5,
    id: 'fixture-editor',
    name: 'Fixture',
    version: '1.0.0',
    main: 'index.html',
    host: { app: 'PS', minVersion: '27.0.0' },
    entrypoints: [{ id: 'panel', type: 'panel' }],
    requiredPermissions: { webview: { allow: 'yes', domains: ['http://127.0.0.1:8188'] } },
  };
}

/** Stage authored fixture files under the same roots used by production tooling. */
function fixture(): string {
  const root = mkdtempSync(path.join(os.tmpdir(), 'sc-uxp-package-'));
  const source = path.join(root, 'integrations/photoshop');
  mkdirSync(path.join(source, 'src/infrastructure'), { recursive: true });
  mkdirSync(path.join(source, 'static'), { recursive: true });
  copyFileSync('integrations/photoshop/build.json', path.join(source, 'build.json'));
  writeFileSync(
    path.join(root, 'tsconfig.photoshop.json'),
    JSON.stringify({ compilerOptions: { target: 'ES2022' } }),
  );
  return root;
}

test('an absent runtime is valid preparation; partial scaffolding fails', async () => {
  const root = fixture();
  try {
    expect(await buildPhotoshop(root, true)).toBe(false);
    writeFileSync(
      path.join(root, 'integrations/photoshop/manifest.json'),
      JSON.stringify(manifest()),
    );
    await expect(buildPhotoshop(root, true)).rejects.toThrow('both manifest.json');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('builds native CommonJS, retains host modules, and leaves deployment untouched during validation', async () => {
  const root = fixture();
  const source = path.join(root, 'integrations/photoshop');
  try {
    writeFileSync(
      path.join(source, 'src/index.ts'),
      'export {name} from "./infrastructure/native.js";',
    );
    writeFileSync(
      path.join(source, 'src/infrastructure/native.ts'),
      'import {host} from "uxp"; export const name=host.name;',
    );
    writeFileSync(
      path.join(source, 'static/index.html'),
      '<html><body><script src="./main.js"></script></body></html>',
    );
    writeFileSync(path.join(source, 'manifest.json'), JSON.stringify(manifest()));
    expect(await buildPhotoshop(root, true)).toBe(true);
    expect(existsSync(path.join(root, 'build/photoshop'))).toBe(false);
    expect(await buildPhotoshop(root, false)).toBe(true);
    const output = path.join(root, 'build/photoshop');
    expect(validatePackage(output, true).id).toBe('fixture-editor');
    expect(readFileSync(path.join(output, 'main.js'), 'utf8')).toContain('require("uxp")');
    writeFileSync(path.join(source, 'src/index.ts'), 'import fs from "node:fs"; console.log(fs);');
    await expect(buildPhotoshop(root, true)).rejects.toThrow();
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}, 20000);

test.each(['../outside', 'C:/outside', '/outside', 'static\\outside'])(
  'rejects unsafe package references: %s',
  (reference) => {
    expect(() => packagePath(path.resolve('.'), reference)).toThrow('Unsafe package path');
  },
);

test('rejects wildcard origins, missing resources and release inspector settings', () => {
  const invalid = manifest();
  invalid.requiredPermissions = { webview: { allow: 'yes', domains: 'all' } };
  expect(() => validateManifest(invalid)).toThrow('explicit HTTP(S) origins');
  const root = fixture();
  try {
    writeFileSync(path.join(root, 'manifest.json'), JSON.stringify(manifest()));
    expect(() => validatePackage(root, true)).toThrow('Missing package resource');
    writeFileSync(path.join(root, 'index.html'), '<webview uxpAllowInspector="true"></webview>');
    expect(() => validatePackage(root, true)).toThrow('inspector');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
