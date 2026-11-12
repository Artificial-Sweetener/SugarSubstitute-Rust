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
 * Keep tests from passing or failing on elapsed time: every test and hook awaits its observed
 * outcome under the shared hang guard, no source sets a timeout of its own, and build work runs
 * once per suite in setup rather than inside test bodies.
 */
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { expect, test } from '@jest/globals';
import { HANG_GUARD_MS } from '../../frontend_core/support/HangGuard.js';

const HANG_GUARD_SOURCE = path.join('tests', 'frontend_core', 'support', 'HangGuard.ts');
const BUILDERS = new Set(['build', 'buildPhotoshop']);
const BUILDER_IMPORT = /import\s*\{([^}]*)\}\s*from\s*'(?:esbuild|[^']*\/photoshop\/build\.js)'/g;
const TOP_LEVEL_HELPER = /^()(?:export\s+)?(?:async\s+)?(?:function|const)\s+(\w+)/;
const TEST_OR_HOOK = /^(\s*)(?:test|it|beforeAll|beforeEach|afterAll|afterEach)(?:\.\w+)?\(/;
const TEST_BODY = /^(\s*)(?:test|it)(?:\.\w+)?\(/;
const STRING_LITERAL = /(['"`])(?:\\.|(?!\1).)*\1/g;
/** A callback followed by a limit argument: `}, 30_000);` or `},\n  LIMIT,\n);`. */
const LIMIT_ARGUMENT = /\}\s*,\s*[\w.]+\s*,?\s*\);?\s*$/;
const LITERAL_TIMEOUTS = [
  /\btimeout\s*:\s*[\d_]/,
  /\b(?:setDefaultTimeout|setDefaultNavigationTimeout)\(\s*[\d_]/,
  /\.setTimeout\(\s*[\d_]/,
  /\bjest\.setTimeout\(/,
];

interface Block {
  readonly line: number;
  readonly name: string;
  readonly text: string;
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

/**
 * Lines that bound a test on elapsed time or build inside a test body: a test or hook given its
 * own limit, a literal timeout option or default, and a test body that bundles or packages.
 */
function wallClockFindings(source: string): number[] {
  const lines = codeLines(source);
  const buildWork = calls(importedBuilders(source));
  const helpers = blocks(lines, TOP_LEVEL_HELPER);
  const ownLimits = blocks(lines, TEST_OR_HOOK).filter((block) => LIMIT_ARGUMENT.test(block.text));
  const buildingTests = blocks(lines, TEST_BODY).filter((body) =>
    doesWork(body, buildWork, helpers),
  );
  const literalTimeouts = lines.flatMap((line, index) =>
    LITERAL_TIMEOUTS.some((pattern) => pattern.test(line)) ? [index + 1] : [],
  );
  return [
    ...new Set([...ownLimits, ...buildingTests].map((block) => block.line).concat(literalTimeouts)),
  ].sort((a, b) => a - b);
}

test('no test, hook, browser wait or child process is bounded by its own elapsed-time limit', () => {
  const findings = testSources()
    .filter((file) => file !== HANG_GUARD_SOURCE)
    .flatMap((file) =>
      wallClockFindings(readFileSync(file, 'utf8')).map((line) => `${file}:${String(line)}`),
    );
  expect(findings).toEqual([]);
});

test('every test and hook runs under the shared hang guard instead of the 5 s Jest default', () => {
  const config: unknown = JSON.parse(readFileSync('jest.config.json', 'utf8'));
  expect(config).toMatchObject({ testTimeout: HANG_GUARD_MS });
});

const BUILDER_IMPORTS = [
  "import { build as bundle } from 'esbuild';",
  "import { buildPhotoshop } from '../../../scripts/photoshop/build.js';",
];

test.each([
  ['a hook with its own limit', ['beforeAll(async () => {', '  await start();', '}, 60_000);']],
  [
    'a test with a shared-constant limit',
    ["test('observes', async () => {", '  await observe();', '}, SETUP_LIMIT_MS);'],
  ],
  [
    'a test.each case with a limit on its own line',
    [
      'test.each([1])(',
      "  'observes %s',",
      '  async () => {',
      '    await observe();',
      '  },',
      '  30_000,',
      ');',
    ],
  ],
  ['a Playwright wait with a literal timeout', ['await option.waitFor({ timeout: 10_000 });']],
  ['a page default timeout literal', ['page.setDefaultTimeout(5_000);']],
  ['a request timeout literal', ['request.setTimeout(5_000, abort);']],
  ['a Jest timeout override', ['jest.setTimeout(30_000);']],
  ['a child process literal timeout', ['spawnSync(node, args, { timeout: 120_000 });']],
  [
    'a bundle inside a test body',
    ["test('observes', async () => {", '  const code = await bundle(options);', '});'],
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
  expect(wallClockFindings([...BUILDER_IMPORTS, ...source].join('\n'))).toEqual([
    BUILDER_IMPORTS.length + 1,
  ]);
});

test('admits setup builds, shared hang guards, quoted fixture code, and other builders', () => {
  expect(
    wallClockFindings(
      [
        ...BUILDER_IMPORTS,
        "const root = 'fixture';",
        'async function packageSource(): Promise<string> {',
        '  await buildPhotoshop(root, false);',
        '}',
        'beforeAll(async () => {',
        '  source = await packageSource();',
        '});',
        'runInNewContext(source, sandbox, { timeout: HANG_GUARD_MS });',
        "test('audits quoted code', () => {",
        "  audit('const run=bundle;bundle(document); page.setDefaultTimeout(5);');",
        '  expect(builder.build(graph)).toEqual([]);',
        '  window.setTimeout(() => settle(), 0);',
        '});',
        "test.each([1])('observes %s', async () => {",
        '  await observe();',
        '});',
      ].join('\n'),
    ),
  ).toEqual([]);
});

test('admits a local helper that shares a builder name without importing a builder', () => {
  expect(
    wallClockFindings(
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
