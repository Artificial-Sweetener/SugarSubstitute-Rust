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
/** Run the adopted native engine contracts with their original isolated Node test runner. */
import { spawnSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
import path from 'node:path';

/** Inventory only the reviewed engine cases within the owning behavioral area. */
function cases(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? cases(file) : entry.name.endsWith('.case.ts') ? [file] : [];
  });
}

test.each(['core/unit', 'services/unit', 'editor/transaction', 'editor/unit'])(
  'preserves adopted %s behavior',
  (group) => {
    const files = cases(path.resolve('tests/prompt_editor/support/adopted', group));
    expect(files.length).toBeGreaterThan(0);
    for (let start = 0; start < files.length; start += 48) {
      const result = spawnSync(
        process.execPath,
        [
          '--experimental-strip-types',
          '--import',
          './tests/prompt_editor/support/AdoptedTypeScriptLoader.ts',
          '--test',
          '--test-reporter=spec',
          '--test-concurrency=4',
          ...files.slice(start, start + 48),
        ],
        { cwd: process.cwd(), encoding: 'utf8', timeout: 120_000, maxBuffer: 4 * 1024 * 1024 },
      );
      if (result.status !== 0)
        throw new Error(
          `Adopted ${group} failed: ${result.error?.message ?? ''}\n${result.stdout.slice(-50_000)}\n${result.stderr}`,
        );
    }
  },
  600_000,
);
