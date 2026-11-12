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
import { runInNewContext } from 'node:vm';
import path from 'node:path';
import { buildPhotoshop } from '../../../scripts/photoshop/build.js';
import {
  packagePath,
  record,
  validateManifest,
  validatePackage,
} from '../../../scripts/photoshop/manifest.js';
import { BUILD_FIXTURE_SETUP_TIMEOUT_MS } from '../../frontend_core/support/BuildFixtureBudget.js';

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

test('release manifest supplies plugin and panel PNG icons at both native densities', () => {
  const authored = record(
    JSON.parse(readFileSync('integrations/photoshop/manifest.json', 'utf8')),
    'manifest',
  );
  if (!Array.isArray(authored.entrypoints)) throw new Error('Missing panel entrypoints.');
  const iconSets: unknown[] = [
    authored.icons,
    ...authored.entrypoints.map((entry: unknown) => record(entry, 'panel').icons),
  ];
  for (const icons of iconSets) {
    expect(Array.isArray(icons)).toBe(true);
    if (!Array.isArray(icons) || !icons.length)
      throw new Error('Release plugin and panel icons are required by the Adobe packager.');
    for (const candidate of icons) {
      const icon = record(candidate, 'icon');
      expect(icon.width).toBe(23);
      expect(icon.height).toBe(23);
      expect(icon.scale).toEqual([1, 2]);
      if (typeof icon.path !== 'string' || !icon.path.endsWith('.png'))
        throw new Error('Photoshop toolbar icons require PNG resources.');
      for (const scale of [1, 2]) {
        const relative = scale === 1 ? icon.path : icon.path.replace(/\.png$/, '@2x.png');
        const bytes = readFileSync(packagePath('integrations/photoshop/static', relative));
        expect(bytes.subarray(0, 8)).toEqual(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
        expect(bytes.readUInt32BE(16)).toBe(23 * scale);
        expect(bytes.readUInt32BE(20)).toBe(23 * scale);
      }
    }
  }
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

/** Stage a complete fixture package whose source files are given relative to the Photoshop root. */
function runtimeFixture(files: Record<string, string>): string {
  const root = fixture();
  const source = path.join(root, 'integrations/photoshop');
  for (const [relative, text] of Object.entries(files))
    writeFileSync(path.join(source, relative), text);
  writeFileSync(
    path.join(source, 'static/index.html'),
    '<html><body><script src="./main.js"></script></body></html>',
  );
  writeFileSync(path.join(source, 'manifest.json'), JSON.stringify(manifest()));
  return root;
}

/** Settle one build so its outcome can be asserted by the test that owns it. */
async function settle(work: Promise<boolean>): Promise<PromiseSettledResult<boolean>> {
  const [outcome] = await Promise.allSettled([work]);
  if (!outcome) throw new Error('Missing build outcome.');
  return outcome;
}

/** Present a settled build outcome to Jest's resolution and rejection matchers. */
function replay(outcome: PromiseSettledResult<boolean>): Promise<boolean> {
  return outcome.status === 'fulfilled'
    ? Promise.resolve(outcome.value)
    : Promise.reject(outcome.reason as Error);
}

const roots = {
  scaffolding: '',
  native: '',
  hostModules: '',
  imageDecoding: '',
};
const builds = {} as Record<
  | 'absentRuntime'
  | 'partialScaffolding'
  | 'nativeValidation'
  | 'nativeDeployment'
  | 'hostModuleValidation'
  | 'imageDecoding',
  PromiseSettledResult<boolean>
>;
let deployedDuringValidation = true;

beforeAll(async () => {
  roots.scaffolding = fixture();
  builds.absentRuntime = await settle(buildPhotoshop(roots.scaffolding, true));
  writeFileSync(
    path.join(roots.scaffolding, 'integrations/photoshop/manifest.json'),
    JSON.stringify(manifest()),
  );
  builds.partialScaffolding = await settle(buildPhotoshop(roots.scaffolding, true));

  roots.native = runtimeFixture({
    'src/index.ts': 'export {name} from "./infrastructure/native.js";',
    'src/infrastructure/native.ts': 'import {host} from "uxp"; export const name=host.name;',
  });
  builds.nativeValidation = await settle(buildPhotoshop(roots.native, true));
  deployedDuringValidation = existsSync(path.join(roots.native, 'build/photoshop'));
  builds.nativeDeployment = await settle(buildPhotoshop(roots.native, false));

  roots.hostModules = runtimeFixture({
    'src/index.ts': 'import fs from "node:fs"; console.log(fs);',
  });
  builds.hostModuleValidation = await settle(buildPhotoshop(roots.hostModules, true));

  roots.imageDecoding = runtimeFixture({
    'src/index.ts': 'import UPNG from "upng-js"; export const decode=UPNG.decode;',
  });
  builds.imageDecoding = await settle(buildPhotoshop(roots.imageDecoding, false));
}, BUILD_FIXTURE_SETUP_TIMEOUT_MS);

afterAll(() => {
  for (const root of Object.values(roots)) if (root) rmSync(root, { recursive: true, force: true });
});

test('an absent runtime is valid preparation; partial scaffolding fails', async () => {
  await expect(replay(builds.absentRuntime)).resolves.toBe(false);
  await expect(replay(builds.partialScaffolding)).rejects.toThrow('both manifest.json');
});

test('builds native CommonJS, retains host modules, and leaves deployment untouched during validation', async () => {
  await expect(replay(builds.nativeValidation)).resolves.toBe(true);
  expect(deployedDuringValidation).toBe(false);
  await expect(replay(builds.nativeDeployment)).resolves.toBe(true);
  const output = path.join(roots.native, 'build/photoshop');
  expect(validatePackage(output, true).id).toBe('fixture-editor');
  expect(readFileSync(path.join(output, 'main.js'), 'utf8')).toContain('require("uxp")');
  await expect(replay(builds.hostModuleValidation)).rejects.toThrow();
});

test.each(['../outside', 'C:/outside', '/outside', 'static\\outside'])(
  'rejects unsafe package references: %s',
  (reference) => {
    expect(() => packagePath(path.resolve('.'), reference)).toThrow('Unsafe package path');
  },
);

test('bundles reviewed image decoding without browser globals and includes its license notices', async () => {
  await expect(replay(builds.imageDecoding)).resolves.toBe(true);
  const output = path.join(roots.imageDecoding, 'build/photoshop');
  expect(readFileSync(path.join(output, 'third_party/NOTICE.md'), 'utf8')).toContain(
    'UPNG.js 2.1.0',
  );
  for (const license of ['upng-js.MIT.txt', 'pako.LICENSE.txt'])
    expect(readFileSync(path.join(output, 'third_party/licenses', license), 'utf8')).toMatch(
      /copyright/i,
    );
  const module: { exports: Record<string, unknown> } = { exports: {} };
  runInNewContext(readFileSync(path.join(output, 'main.js'), 'utf8'), {
    module,
    require: (name: string): never => {
      throw new Error(`Unexpected unbundled dependency: ${name}`);
    },
  });
  expect(typeof module.exports.decode).toBe('function');
});

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
