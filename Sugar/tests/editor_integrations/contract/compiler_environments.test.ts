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
/** Prove native and portable source trees receive only their intended compiler APIs. */
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import ts from 'typescript';

/** Compile a real probe against the repository's selected environment without emitting code. */
function errors(configName: string, source: string): readonly ts.Diagnostic[] {
  const root = path.resolve('.');
  const parent = path.join(root, '.generated');
  mkdirSync(parent, { recursive: true });
  const directory = mkdtempSync(path.join(parent, 'editor-types-'));
  try {
    const probe = path.join(directory, 'probe.ts');
    writeFileSync(probe, source);
    const config = ts.readConfigFile(path.join(root, configName), ts.sys.readFile);
    const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, root);
    return ts.getPreEmitDiagnostics(ts.createProgram([probe], { ...parsed.options, noEmit: true }));
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test('portable code has no browser, Node or Photoshop ambient APIs', () => {
  expect(
    errors('tsconfig.integrations.json', 'export interface Context { readonly id:string; }'),
  ).toEqual([]);
  for (const source of [
    'document.createElement("div");',
    'process.cwd();',
    'require("photoshop");',
  ])
    expect(errors('tsconfig.integrations.json', source).length).toBeGreaterThan(0);
});

test('native code receives pinned Photoshop and UXP types without Node globals', () => {
  expect(
    errors(
      'tsconfig.photoshop.json',
      'import {app} from "photoshop"; import {host} from "uxp"; void app.documents; void host.name;',
    ),
  ).toEqual([]);
  expect(errors('tsconfig.photoshop.json', 'process.cwd();').length).toBeGreaterThan(0);
});
