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
/** Prove portable ownership across multiple editor identities and module-loading forms. */
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  auditIntegrations,
  type IntegrationBoundaries,
} from '../../../scripts/integrations/audit.js';

const policy: IntegrationBoundaries = {
  contracts: 'integrations/contracts',
  shared: 'integrations/shared',
  editors: [
    {
      name: 'photoshop',
      root: 'integrations/photoshop',
      infrastructure: 'integrations/photoshop/src/infrastructure',
      modules: ['photoshop', 'uxp'],
    },
    {
      name: 'krita',
      root: 'integrations/krita',
      infrastructure: 'integrations/krita/src/infrastructure',
      modules: ['krita'],
    },
  ],
};

/** Audit fixture modules through real TypeScript path resolution without executing host code. */
function audit(file: string, source: string): string[] {
  const root = mkdtempSync(path.join(os.tmpdir(), 'sc-editor-boundary-'));
  try {
    const files: Record<string, string> = {
      'tsconfig.json': JSON.stringify({
        compilerOptions: {
          baseUrl: '.',
          paths: { '@native': ['integrations/photoshop/src/infrastructure/native.ts'] },
        },
      }),
      'integrations/photoshop/src/infrastructure/native.ts': 'export const value = 1;',
      'integrations/contracts/values.ts': 'export interface DocumentIdentity { id: string; }',
      [file]: source,
    };
    for (const [relative, content] of Object.entries(files)) {
      const target = path.join(root, relative);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
    return auditIntegrations(root, [file], policy);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test.each([
  'import {app} from "photoshop";',
  'const api = require("photoshop");',
  'export * from "uxp";',
  'type Host = import("photoshop").App;',
  'const api = import("photoshop");',
  'import native = require("photoshop");',
  'const loader = require; loader("photoshop");',
  'import {value} from "@native";',
])('rejects native access from portable code: %s', (source) => {
  expect(audit('integrations/shared/session.ts', source).length).toBeGreaterThan(0);
});

test('allows each host to implement contracts through its own native API owner', () => {
  expect(
    audit(
      'integrations/photoshop/src/infrastructure/native.ts',
      'import {app} from "photoshop"; import type {DocumentIdentity} from "../../../contracts/values.js";',
    ),
  ).toEqual([]);
  expect(
    audit('integrations/krita/src/infrastructure/native.ts', 'const app = require("krita");'),
  ).toEqual([]);
});

test('rejects cross-editor implementations, ComfyUI dependencies and unregistered editors', () => {
  expect(
    audit('integrations/krita/src/infrastructure/native.ts', 'import {value} from "@native";')
      .length,
  ).toBeGreaterThan(0);
  expect(
    audit('integrations/photoshop/src/application/session.ts', 'import {app} from "photoshop";')
      .length,
  ).toBeGreaterThan(0);
  expect(
    audit('frontend/comfyui/panel.ts', 'import {value} from "@native";').length,
  ).toBeGreaterThan(0);
  expect(
    audit('integrations/shared/session.ts', 'import {app} from "/scripts/app.js";').length,
  ).toBeGreaterThan(0);
  expect(audit('integrations/gimp/src/native.ts', 'export {};').length).toBeGreaterThan(0);
});

test('keeps contract imports one-way and rejects dynamic module discovery', () => {
  expect(
    audit('integrations/photoshop/src/application/session.ts', 'import {value} from "@native";')
      .length,
  ).toBeGreaterThan(0);
  expect(
    audit('integrations/contracts/model.ts', 'import "../shared/session.js";').length,
  ).toBeGreaterThan(0);
  expect(
    audit(
      'integrations/shared/session.ts',
      'import type {DocumentIdentity} from "../contracts/values.js";',
    ),
  ).toEqual([]);
  expect(
    audit('integrations/shared/session.ts', 'declare const moduleName:string; import(moduleName);')
      .length,
  ).toBeGreaterThan(0);
});
