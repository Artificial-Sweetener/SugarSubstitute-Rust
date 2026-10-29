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
/** Qualify generated validation ownership, public exports and rejection of invalid module cuts. */
import { runInNewContext } from 'node:vm';
import { ModuleKind, transpileModule } from 'typescript';
import { contractBindings } from '../../../scripts/integrations/ContractBindings.js';
import type { ContractValidationModule } from '../../../scripts/integrations/ContractValidationModules.js';

const schema = {
  $defs: {
    FixturePoint: {
      type: 'object',
      properties: { x: { type: 'integer', minimum: 0, maximum: 10 } },
      required: ['x'],
      additionalProperties: false,
    },
    FixtureCamera: {
      type: 'object',
      properties: { point: { $ref: '#/$defs/FixturePoint' } },
      required: ['point'],
      additionalProperties: false,
    },
    FixtureRequest: {
      type: 'object',
      properties: { type: { const: 'request' }, camera: { $ref: '#/$defs/FixtureCamera' } },
      required: ['type', 'camera'],
      additionalProperties: false,
    },
  },
  oneOf: [{ $ref: '#/$defs/FixtureRequest' }],
};
const options = {
  union: 'FixtureMessage',
  schema: 'fixture.schema.json',
  generator: 'scripts/integrations/generate-fixture-contract.ts',
  types: 'types.ts',
  validation: 'validation.ts',
  importPath: './types.js',
  description: 'the fixture',
};
const modules: readonly ContractValidationModule[] = [
  { file: 'Geometry.generated.ts', definitions: ['FixturePoint'] },
  { file: 'Viewport.generated.ts', definitions: ['FixtureCamera', 'FixtureRequest'] },
];

/** Compile real generated sibling imports before exercising every public guard. */
function executable(
  sources: Readonly<Record<string, string>>,
): Record<string, (value: unknown) => boolean> {
  const cache = new Map<string, Record<string, (value: unknown) => boolean>>();
  /** Resolve only the generated fixture's sibling modules. */
  function load(file: string): Record<string, (value: unknown) => boolean> {
    const existing = cache.get(file);
    if (existing) return existing;
    const source = sources[file];
    if (!source) throw new Error(`Missing generated module ${file}.`);
    const exports: Record<string, (value: unknown) => boolean> = {};
    cache.set(file, exports);
    runInNewContext(
      transpileModule(source, { compilerOptions: { module: ModuleKind.CommonJS } }).outputText,
      {
        exports,
        require: (name: string) => load(name.replace(/^\.\//, '').replace(/\.js$/, '.ts')),
      },
    );
    return exports;
  }
  return load('validation.ts');
}

test('preserves public types and every runtime guard across acyclic contract modules', async () => {
  const text = JSON.stringify(schema);
  const plain = await contractBindings(text, options);
  const split = await contractBindings(text, { ...options, validationModules: modules });
  expect(split['types.ts']).toBe(plain['types.ts']);
  expect(split['Viewport.generated.ts']).toContain("from './Geometry.generated.js'");
  expect(split['Viewport.generated.ts']).not.toContain("from './validation.js'");
  expect(split['validation.ts']).not.toContain('function isRecord');
  const expected = executable(plain),
    actual = executable(split);
  expect(Object.keys(actual).sort()).toEqual(Object.keys(expected).sort());
  for (const name of Object.keys(expected)) {
    const left = expected[name],
      right = actual[name];
    if (!left || !right) throw new Error('Missing public generated guard.');
    for (const value of [
      null,
      [],
      {},
      { x: 1 },
      { x: 11 },
      { point: { x: 0 } },
      { type: 'request', camera: { point: { x: 10 } } },
      { type: 'request', camera: { point: { x: 11 } } },
      { type: 'request', camera: { point: { x: 1, extra: true } } },
    ])
      expect(right(value)).toBe(left(value));
  }
});

test.each(
  [
    [{ file: 'Geometry.generated.ts', definitions: ['Unknown'] }],
    [{ file: 'Geometry.generated.ts', definitions: ['FixtureMessage'] }],
    [{ file: 'Geometry.generated.ts', definitions: [] }],
    [{ file: '../Geometry.generated.ts', definitions: ['FixturePoint'] }],
    [{ file: 'not-code.txt', definitions: ['FixturePoint'] }],
    [{ file: 'Geometry.generated.ts', definitions: ['FixturePoint', 'FixturePoint'] }],
    [
      { file: 'Geometry.generated.ts', definitions: ['FixturePoint'] },
      { file: 'Viewport.generated.ts', definitions: ['FixturePoint'] },
    ],
    [
      { file: 'Geometry.generated.ts', definitions: ['FixturePoint'] },
      { file: 'Geometry.generated.ts', definitions: ['FixtureCamera'] },
    ],
    [{ file: 'Viewport.generated.ts', definitions: ['FixtureRequest'] }],
  ].map((validationModules) => ({ validationModules })),
)(
  'rejects unknown owners, invalid paths and cyclic module cuts %#',
  async ({ validationModules }) => {
    await expect(
      contractBindings(JSON.stringify(schema), { ...options, validationModules }),
    ).rejects.toThrow(/generated|definitions|Cyclic/);
  },
);

test('rejects a validator module that overwrites generated type declarations', async () => {
  await expect(
    contractBindings(JSON.stringify(schema), {
      ...options,
      types: 'Geometry.generated.ts',
      validationModules: modules,
    }),
  ).rejects.toThrow('validation module');
});

test('includes facade reexports when rejecting cycles through an otherwise unused validator', async () => {
  await expect(
    contractBindings(
      JSON.stringify({
        ...schema,
        oneOf: [{ $ref: '#/$defs/FixturePoint' }],
      }),
      {
        ...options,
        validationModules: [{ file: 'Viewport.generated.ts', definitions: ['FixtureCamera'] }],
      },
    ),
  ).rejects.toThrow('Cyclic');
});
