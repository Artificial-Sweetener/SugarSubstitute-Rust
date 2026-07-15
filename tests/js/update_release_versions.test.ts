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
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

import { updateReleaseVersions } from '../../scripts/update-release-versions.js';

describe('updateReleaseVersions', () => {
  let rootPath: string;

  beforeEach(() => {
    rootPath = mkdtempSync(join(tmpdir(), 'sugarcubes-release-'));
    writeFileSync(
      join(rootPath, 'package.json'),
      `${JSON.stringify({ name: 'SugarCubes', version: '0.9.0' }, null, 2)}\n`,
      'utf8',
    );
    writeFileSync(
      join(rootPath, 'package-lock.json'),
      `${JSON.stringify(
        {
          name: 'SugarCubes',
          version: '0.9.0',
          packages: { '': { name: 'SugarCubes', version: '0.9.0' } },
        },
        null,
        2,
      )}\n`,
      'utf8',
    );
    writeFileSync(join(rootPath, 'pyproject.toml'), 'version = "0.9.0"\n', 'utf8');
    writeFileSync(join(rootPath, '__init__.py'), '__version__ = "0.9.0"\n', 'utf8');
  });

  afterEach(() => {
    rmSync(rootPath, { recursive: true, force: true });
  });

  test('updates every release version owner', () => {
    updateReleaseVersions(pathToFileURL(`${rootPath}/`), '0.10.0');

    expect(JSON.parse(readFileSync(join(rootPath, 'package.json'), 'utf8')).version).toBe('0.10.0');
    const lockfile = JSON.parse(readFileSync(join(rootPath, 'package-lock.json'), 'utf8'));
    expect(lockfile.version).toBe('0.10.0');
    expect(lockfile.packages[''].version).toBe('0.10.0');
    expect(readFileSync(join(rootPath, 'pyproject.toml'), 'utf8')).toContain('version = "0.10.0"');
    expect(readFileSync(join(rootPath, '__init__.py'), 'utf8')).toContain('__version__ = "0.10.0"');
  });
});
