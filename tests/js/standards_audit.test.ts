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
import { describe, expect, test } from '@jest/globals';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { auditStandards } from '../../scripts/standards-audit.js';

describe('standards audit', () => {
  test('frontend modules and tooling comply with the baseline standards', () => {
    const failures = auditStandards(path.resolve('.'));
    expect(failures).toEqual([]);
  });

  test('rejects source and runtime files that cross the generated boundary', () => {
    const fixtureRoot = mkdtempSync(path.join(os.tmpdir(), 'sugarcubes-frontend-boundary-'));
    try {
      mkdirSync(path.join(fixtureRoot, 'frontend'), { recursive: true });
      mkdirSync(path.join(fixtureRoot, 'web'), { recursive: true });
      mkdirSync(path.join(fixtureRoot, 'scripts'), { recursive: true });
      writeFileSync(path.join(fixtureRoot, 'frontend', 'authored.js'), 'export {};\n');
      writeFileSync(path.join(fixtureRoot, 'web', 'generated.ts'), 'export {};\n');

      expect(auditStandards(fixtureRoot)).toEqual([
        'frontend/authored.js is JavaScript in the TypeScript source tree',
        'web/generated.ts is TypeScript in the generated runtime tree',
      ]);
    } finally {
      rmSync(fixtureRoot, { force: true, recursive: true });
    }
  });
});
