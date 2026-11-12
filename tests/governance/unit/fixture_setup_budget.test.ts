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
/** Require every setup hook that bundles source or launches a browser to declare a shared budget. */
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { expect, test } from '@jest/globals';

const SHARED_BUDGETS = ['BROWSER_FIXTURE_SETUP_TIMEOUT_MS', 'BUILD_FIXTURE_SETUP_TIMEOUT_MS'];
const HEAVY_SETUP = /\bawait\s+(?:build|launchHeadlessBrowser|startPromptBrowser)\(/;

/** Every authored test source. */
function testSources(folder = 'tests'): string[] {
  return readdirSync(folder, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(folder, entry.name);
    if (entry.isDirectory()) return testSources(file);
    return entry.name.endsWith('.ts') ? [file] : [];
  });
}

/** Name each setup hook that does build or launch work without one of the shared budgets. */
function unbudgetedSetupHooks(source: string): number[] {
  const lines = source.split('\n');
  const findings: number[] = [];
  for (let start = 0; start < lines.length; start += 1) {
    const hook = /^(\s*)(?:beforeAll|beforeEach)\(/.exec(lines[start] ?? '');
    if (!hook) continue;
    const indent = hook[1] ?? '';
    let end = start + 1;
    while (
      end < lines.length &&
      !new RegExp(`^${indent}\\}(?:\\)|, *[\\w_]+\\));`).test(lines[end] ?? '')
    )
      end += 1;
    const body = lines.slice(start, end + 1).join('\n');
    const close = lines[end] ?? '';
    if (HEAVY_SETUP.test(body) && !SHARED_BUDGETS.some((budget) => close.includes(budget)))
      findings.push(start + 1);
    start = end;
  }
  return findings;
}

test('every setup hook that bundles source or launches a browser declares a shared budget', () => {
  const findings = testSources().flatMap((file) =>
    unbudgetedSetupHooks(readFileSync(file, 'utf8')).map((line) => `${file}:${String(line)}`),
  );
  expect(findings).toEqual([]);
});

test.each([
  [
    'a bundle on the default budget',
    'beforeAll(async () => {\n  code = await build(options);\n});',
  ],
  [
    'a browser launch on a literal budget',
    'beforeAll(async () => {\n  browser = await launchHeadlessBrowser(chromium);\n}, 5000);',
  ],
])('rejects %s', (_label, source) => {
  expect(unbudgetedSetupHooks(source)).toEqual([1]);
});

test('admits shared budgets and setup without build or launch work', () => {
  expect(
    unbudgetedSetupHooks(
      [
        'beforeAll(async () => {',
        '  code = await build(options);',
        '}, BUILD_FIXTURE_SETUP_TIMEOUT_MS);',
        'beforeEach(() => {',
        '  reset();',
        '});',
      ].join('\n'),
    ),
  ).toEqual([]);
});
