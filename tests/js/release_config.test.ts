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
import { readFileSync } from 'node:fs';

interface ReleaseConfig {
  plugins: Array<string | [string, { releaseRules?: unknown[] }]>;
}

const releaseConfig = JSON.parse(
  readFileSync(new URL('../../.releaserc.json', import.meta.url), 'utf8'),
) as ReleaseConfig;

describe('semantic-release configuration', () => {
  test('keeps breaking changes on the 0.x release line', () => {
    const commitAnalyzer = releaseConfig.plugins.find(
      (plugin) => Array.isArray(plugin) && plugin[0] === '@semantic-release/commit-analyzer',
    );

    expect(commitAnalyzer).toBeDefined();
    expect(Array.isArray(commitAnalyzer) ? commitAnalyzer[1].releaseRules : undefined).toEqual(
      expect.arrayContaining([{ breaking: true, release: 'minor' }]),
    );
  });
});
