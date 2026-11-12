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
/**
 * Keep build and launch work in setup hooks on a shared budget, so test bodies only observe its
 * result within their own limits.
 */
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { expect, test } from '@jest/globals';

const SHARED_BUDGETS = ['BROWSER_FIXTURE_SETUP_TIMEOUT_MS', 'BUILD_FIXTURE_SETUP_TIMEOUT_MS'];
const BUILDERS = new Set(['build', 'buildPhotoshop']);
const BUILDER_IMPORT = /import\s*\{([^}]*)\}\s*from\s*'(?:esbuild|[^']*\/photoshop\/build\.js)'/g;
const LAUNCHERS = ['launchHeadlessBrowser', 'startPromptBrowser'];
const TOP_LEVEL_HELPER = /^()(?:export\s+)?(?:async\s+)?(?:function|const)\s+(\w+)/;
const SETUP_HOOK = /^(\s*)(?:beforeAll|beforeEach)\(/;
const TEST_BODY = /^(\s*)(?:test|it)(?:\.\w+)?\(/;
const STRING_LITERAL = /(['"`])(?:\\.|(?!\1).)*\1/g;

interface Block {
  readonly line: number;
  readonly name: string;
  readonly text: string;
  readonly close: string;
}

/** Every authored test source. */
function testSources(folder = 'tests'): string[] {
  return readdirSync(folder, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(folder, entry.name);
    if (entry.isDirectory()) return testSources(file);
    return entry.name.endsWith('.ts') ? [file] : [];
  });
}

/** Source lines with string literal contents removed, so quoted fixture code is not mistaken for work. */
function codeLines(source: string): string[] {
  return source.split('\n').map((line) => line.replace(STRING_LITERAL, "''"));
}

/**
 * Each block opened by `start`: a single line ending in `;`, or through the next line at the
 * opening indent that begins with `}` or `)`.
 */
function blocks(lines: string[], start: RegExp): Block[] {
  const found: Block[] = [];
  for (let open = 0; open < lines.length; open += 1) {
    const first = lines[open] ?? '';
    const match = start.exec(first);
    if (!match) continue;
    const close = new RegExp(`^${match[1] ?? ''}[})]`);
    let end = open;
    if (!first.trimEnd().endsWith(';'))
      do end += 1;
      while (end < lines.length && !close.test(lines[end] ?? ''));
    found.push({
      line: open + 1,
      name: match[2] ?? '',
      text: lines.slice(open, end + 1).join('\n'),
      close: lines[end] ?? '',
    });
    open = end;
  }
  return found;
}

/** Local names under which a source imports esbuild's or the Photoshop package's build. */
function importedBuilders(source: string): string[] {
  return [...source.matchAll(BUILDER_IMPORT)].flatMap((statement) =>
    (statement[1] ?? '').split(',').flatMap((specifier) => {
      const [imported = '', local = imported] = specifier.trim().split(/\s+as\s+/);
      return BUILDERS.has(imported) ? [local] : [];
    }),
  );
}

/** A pattern matching a direct call of any of `names`, or nothing when there are none. */
function calls(names: string[]): RegExp {
  return names.length ? new RegExp(`(?<![.\\w])(?:${names.join('|')})\\(`) : /(?!)/;
}

/** Whether a block does `work` directly or through a top-level helper of the same source that does. */
function doesWork(block: Block, work: RegExp, helpers: Block[]): boolean {
  return (
    work.test(block.text) ||
    helpers.some(
      (helper) =>
        work.test(helper.text) && new RegExp(`(?<![.\\w])${helper.name}\\(`).test(block.text),
    )
  );
}

/** Lines of setup hooks that do build or launch work without a shared budget, and of test bodies that build. */
function misplacedFixtureWork(source: string): number[] {
  const lines = codeLines(source);
  const builders = importedBuilders(source);
  const buildWork = calls(builders);
  const setupWork = calls([...builders, ...LAUNCHERS]);
  const helpers = blocks(lines, TOP_LEVEL_HELPER);
  const unbudgetedSetup = blocks(lines, SETUP_HOOK).filter(
    (hook) =>
      doesWork(hook, setupWork, helpers) &&
      !SHARED_BUDGETS.some((budget) => hook.close.includes(budget)),
  );
  const buildingTests = blocks(lines, TEST_BODY).filter((body) =>
    doesWork(body, buildWork, helpers),
  );
  return [...unbudgetedSetup, ...buildingTests].map((block) => block.line).sort((a, b) => a - b);
}

test('build and launch work runs only in setup hooks that declare a shared budget', () => {
  const findings = testSources().flatMap((file) =>
    misplacedFixtureWork(readFileSync(file, 'utf8')).map((line) => `${file}:${String(line)}`),
  );
  expect(findings).toEqual([]);
});

const BUILDER_IMPORTS = [
  "import { build as bundle } from 'esbuild';",
  "import { buildPhotoshop } from '../../../scripts/photoshop/build.js';",
];

test.each([
  [
    'a bundle on the default budget',
    ['beforeAll(async () => {', '  code = await bundle(options);', '});'],
  ],
  [
    'a browser launch on a literal budget',
    ['beforeAll(async () => {', '  browser = await launchHeadlessBrowser(chromium);', '}, 5000);'],
  ],
  [
    'a bundle inside a test body',
    ["test('observes', async () => {", '  const code = await bundle(options);', '}, 20000);'],
  ],
  [
    'a package build inside a test body',
    [
      "test('observes', async () => {",
      '  expect(await buildPhotoshop(root, true)).toBe(true);',
      '});',
    ],
  ],
  [
    'a test body calling a helper that packages',
    [
      "test('observes', async () => {",
      '  const source = await packageSource();',
      '});',
      'async function packageSource(): Promise<string> {',
      '  await buildPhotoshop(root, false);',
      '}',
    ],
  ],
])('rejects %s', (_label, source) => {
  expect(misplacedFixtureWork([...BUILDER_IMPORTS, ...source].join('\n'))).toEqual([
    BUILDER_IMPORTS.length + 1,
  ]);
});

test('admits budgeted setup, quoted fixture code, and tests that use fakes or other builders', () => {
  expect(
    misplacedFixtureWork(
      [
        ...BUILDER_IMPORTS,
        "const root = 'fixture';",
        'async function packageSource(): Promise<string> {',
        '  await buildPhotoshop(root, false);',
        '}',
        'beforeAll(async () => {',
        '  source = await packageSource();',
        '}, BUILD_FIXTURE_SETUP_TIMEOUT_MS);',
        'beforeEach(() => {',
        '  reset();',
        '});',
        "test('audits quoted code', () => {",
        "  audit('const run=bundle;bundle(document);');",
        '  expect(builder.build(graph)).toEqual([]);',
        '  launchHeadlessBrowser(fakeEngine);',
        '});',
      ].join('\n'),
    ),
  ).toEqual([]);
});

test('admits a local helper that shares a builder name without importing a builder', () => {
  expect(
    misplacedFixtureWork(
      [
        'function build(root: Element): void {',
        '  root.append(surface);',
        '}',
        "test('clips', () => {",
        '  build(root);',
        '});',
      ].join('\n'),
    ),
  ).toEqual([]);
});
