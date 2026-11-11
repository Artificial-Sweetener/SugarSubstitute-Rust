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
/** Prove browser bundles are identical with installed and attached dependency directories. */
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  renameSync,
  symlinkSync,
  unlinkSync,
  rmdirSync,
  rmSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { build } from 'esbuild';
import { browserDependencyResolution } from '../../../scripts/BrowserBuildEnvironment.js';

test('preserves browser bundle bytes when a snapshot attaches the dependency installation', async () => {
  const fixture = mkdtempSync(path.join(os.tmpdir(), 'sc-browser-dependencies-'));
  const root = path.join(fixture, 'project');
  const dependencies = path.join(root, 'node_modules');
  let attached = false;
  try {
    mkdirSync(path.join(dependencies, 'fixture'), { recursive: true });
    writeFileSync(path.join(dependencies, 'fixture/package.json'), '{"main":"index.js"}');
    writeFileSync(path.join(dependencies, 'fixture/index.js'), 'export const answer=42;');
    writeFileSync(path.join(root, 'entry.js'), 'export {answer} from "fixture";');
    const compile = async (): Promise<string[]> => {
      const result = await build({
        ...browserDependencyResolution,
        absWorkingDir: root,
        entryPoints: ['entry.js'],
        outdir: 'output',
        bundle: true,
        write: false,
        format: 'esm',
        platform: 'browser',
      });
      return result.outputFiles.map((file) => file.text);
    };
    const installed = await compile();
    const external = path.join(fixture, 'installed');
    renameSync(dependencies, external);
    symlinkSync(external, dependencies, process.platform === 'win32' ? 'junction' : 'dir');
    attached = true;
    expect(await compile()).toEqual(installed);
  } finally {
    if (attached) {
      if (process.platform === 'win32') rmdirSync(dependencies);
      else unlinkSync(dependencies);
    }
    rmSync(fixture, { recursive: true, force: true });
  }
}, 20000);
