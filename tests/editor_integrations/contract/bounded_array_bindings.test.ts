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
/** Qualify bounded enum arrays and fail-closed schema admission across generated languages. */

import { runInNewContext } from 'node:vm';
import { ModuleKind, transpileModule } from 'typescript';
import { contractBindings } from '../../../scripts/integrations/ContractBindings.js';
import { contractArraySchema } from '../../../scripts/integrations/ContractArraySchema.js';
import { rustContractBindings } from '../../../scripts/integrations/RustContractBindings.js';

type Schema = Record<string, unknown>;
const options = {
  schema: 'fixture.schema.json',
  generator: 'scripts/integrations/generate-fixture-contract.ts',
  prefix: 'Fixture',
};
const array: Schema = {
  type: 'array',
  items: { enum: ['fit', 'native', 'zoom', 'pan'] },
  maxItems: 4,
};

/** Exercise array properties through the same closed-object contracts used in production. */
function definitions(items: Schema, extra: Schema = {}): Schema {
  return {
    ...extra,
    FixtureCapabilities: {
      type: 'object',
      properties: { commands: items },
      required: ['commands'],
      additionalProperties: false,
    },
  };
}

/** Generate Rust using raw schema nodes so invalid numeric bounds remain observable. */
function rust(items: Schema): string {
  const values = definitions(items);
  return rustContractBindings(values, JSON.stringify(values), options);
}

/** Generate portable TypeScript declarations and validators from a fixture schema. */
async function typescript(
  items: Schema,
  extra: Schema = {},
): Promise<Readonly<Record<string, string>>> {
  return contractBindings(
    JSON.stringify({ $defs: definitions(items, extra), $ref: '#/$defs/FixtureCapabilities' }),
    {
      ...options,
      union: 'FixtureMessage',
      types: 'types.ts',
      validation: 'validation.ts',
      importPath: './types.js',
      description: 'the bounded-array fixture',
    },
  );
}

/** Execute the actual generated guard after erasing its compile-time-only imports. */
async function validator(items: Schema, extra: Schema = {}): Promise<(value: unknown) => boolean> {
  const outputs = await typescript(items, extra);
  const source = outputs['validation.ts'];
  if (!source) throw new Error('Missing generated fixture validator.');
  const exports: Record<string, unknown> = {};
  runInNewContext(
    transpileModule(source, { compilerOptions: { module: ModuleKind.CommonJS } }).outputText,
    {
      exports,
    },
  );
  const guard = exports.isFixtureMessage;
  if (typeof guard !== 'function') throw new Error('Missing fixture root guard.');
  return (value) => guard(value) === true;
}

test('generates bounded enum vectors and validates every advertised token', async () => {
  const outputs = await typescript(array);
  expect(outputs['types.ts']).toContain(
    "readonly commands: readonly ('fit' | 'native' | 'zoom' | 'pan')[];",
  );
  const valid = await validator(array);
  for (const commands of [[], ['fit'], ['fit', 'native', 'zoom', 'pan'], ['fit', 'fit']])
    expect(valid({ commands })).toBe(true);
  for (const value of [
    {},
    null,
    { commands: 'fit' },
    { commands: ['unknown'] },
    { commands: ['fit', 1] },
    { commands: ['fit', null] },
    { commands: ['fit', {}] },
    { commands: ['fit', 'native', 'zoom', 'pan', 'fit'] },
    { commands: ['fit'], extra: true },
  ])
    expect(valid(value)).toBe(false);
  const source = rust(array);
  expect(source).toContain('pub(super) commands: Vec<String>');
  expect(source).toContain('valid_fixture_capabilities_commands(&self.commands)');
  expect(source).toContain('if value.len() > 4 {');
  expect(source).toContain('for item in value {');
  expect(source).toContain('item.as_str(),');
  for (const command of ['fit', 'native', 'zoom', 'pan']) expect(source).toContain(`"${command}"`);
});

test.each([0, 4096])('preserves the supported collection boundary %i', async (maxItems) => {
  const bounded = { ...array, maxItems };
  const valid = await validator(bounded);
  expect(valid({ commands: Array.from({ length: maxItems }, () => 'fit') })).toBe(true);
  expect(valid({ commands: Array.from({ length: maxItems + 1 }, () => 'fit') })).toBe(false);
  expect(rust(bounded)).toContain(
    maxItems ? 'if value.len() > 4096 {' : 'self.commands.is_empty()',
  );
});

test.each([
  undefined,
  null,
  '4',
  -1,
  0.5,
  Number.NaN,
  Number.POSITIVE_INFINITY,
  4097,
  Number.MAX_SAFE_INTEGER + 1,
])('refuses a missing or invalid maximum %s in both generators', async (maxItems) => {
  const invalid = { ...array, maxItems };
  expect(() => contractArraySchema(invalid)).toThrow('finite integer bound');
  expect(() => rust(invalid)).toThrow('finite integer bound');
  await expect(typescript(invalid)).rejects.toThrow('finite integer bound');
});

test.each([
  { uniqueItems: true },
  { additionalItems: false },
  { contains: { const: 'fit' } },
  { enum: ['fit'] },
  { $ref: '#/$defs/FixtureCapabilities' },
])('refuses unsupported array constraints %p before generating bindings', async (keyword) => {
  const invalid = { ...array, ...keyword };
  expect(() => rust(invalid)).toThrow('Unsupported contract array schema keyword');
  await expect(typescript(invalid)).rejects.toThrow('Unsupported contract array schema keyword');
});

test.each([undefined, null, true, [], [{ enum: ['fit'] }]])(
  'refuses missing, dynamic and tuple item shapes %p',
  async (items) => {
    const invalid = { ...array, items };
    expect(() => rust(invalid)).toThrow('Invalid contract array item schema');
    await expect(typescript(invalid)).rejects.toThrow('Invalid contract array item schema');
  },
);

test.each([
  { enum: [] },
  { enum: ['fit', 'fit'] },
  { enum: [1] },
  { enum: ['fit', null] },
  { enum: ['FIT'] },
  { enum: ['fit"'] },
  { enum: ['fit\n'] },
  { enum: 'fit' },
  { enum: ['fit'], type: 'string' },
  { enum: ['fit'], maxLength: 3 },
  { enum: ['fit'], $ref: '#/$defs/FixtureCapabilities' },
])('refuses unsupported enum items %p consistently', async (items) => {
  const invalid = { ...array, items };
  expect(() => rust(invalid)).toThrow('closed string enum');
  await expect(typescript(invalid)).rejects.toThrow('closed string enum');
});

test.each([
  {},
  { type: 'string', maxLength: 4 },
  { $ref: '#/$defs/FixtureUnknown' },
  { type: 'object', properties: {}, required: [], additionalProperties: false },
  { type: 'array', items: { enum: ['fit'] }, maxItems: 1 },
])('keeps unsupported Rust array item vocabularies fail closed: %p', (items) => {
  expect(() => rust({ ...array, items })).toThrow('closed string enum');
});

test('preserves TypeScript arrays of bounded objects and local references', async () => {
  const item = {
    type: 'object',
    properties: { name: { type: 'string', maxLength: 4 } },
    required: ['name'],
    additionalProperties: false,
  };
  for (const items of [item, { $ref: '#/$defs/FixtureElement' }]) {
    const valid = await validator(
      { type: 'array', items, maxItems: 4096 },
      { FixtureElement: item },
    );
    expect(valid({ commands: [] })).toBe(true);
    expect(valid({ commands: [{ name: 'fit' }] })).toBe(true);
    expect(valid({ commands: [{ name: 'large' }] })).toBe(false);
    expect(valid({ commands: [{ name: 'fit', extra: true }] })).toBe(false);
    expect(valid({ commands: Array.from({ length: 4097 }, () => ({ name: 'fit' })) })).toBe(false);
  }
});

test.each([0, -1, 0.5, 5, '1', null, Number.NaN])(
  'refuses an invalid minimum %p in both generators',
  async (minItems) => {
    const invalid = { ...array, minItems };
    expect(() => contractArraySchema(invalid)).toThrow('positive integers within the maximum');
    expect(() => rust(invalid)).toThrow('positive integers within the maximum');
    await expect(typescript(invalid)).rejects.toThrow('positive integers within the maximum');
  },
);

test('requires nonempty collections of named objects in both generated languages', async () => {
  const element = {
    type: 'object',
    properties: { name: { type: 'string', maxLength: 4 } },
    required: ['name'],
    additionalProperties: false,
  };
  const items = {
    type: 'array',
    items: { $ref: '#/$defs/FixtureElement' },
    minItems: 1,
    maxItems: 8,
  };
  const valid = await validator(items, { FixtureElement: element });
  expect(valid({ commands: [] })).toBe(false);
  expect(valid({ commands: [{ name: 'fit' }] })).toBe(true);
  expect(valid({ commands: Array.from({ length: 8 }, () => ({ name: 'fit' })) })).toBe(true);
  expect(valid({ commands: Array.from({ length: 9 }, () => ({ name: 'fit' })) })).toBe(false);
  expect(valid({ commands: [{ name: 'large' }] })).toBe(false);
  const values = definitions(items, { FixtureElement: element });
  const source = rustContractBindings(values, JSON.stringify(values), options);
  expect(source).toContain('pub(super) commands: Vec<FixtureElement>,');
  expect(source).toContain('!self.commands.is_empty()');
  expect(source).toContain('self.commands.len() <= 8');
  expect(source).toContain('self.commands.iter().all(FixtureElement::valid)');
  const pair = { ...items, minItems: 2 };
  const pairs = definitions(pair, { FixtureElement: element });
  expect(rustContractBindings(pairs, JSON.stringify(pairs), options)).toContain(
    'self.commands.len() >= 2',
  );
  expect((await validator(pair, { FixtureElement: element }))({ commands: [{ name: 'a' }] })).toBe(
    false,
  );
  expect(() => rust({ ...array, minItems: 1 })).toThrow('do not support a minimum length');
});

test('formats longer enum lists without widening Rust source limits', () => {
  const source = rust({
    type: 'array',
    items: {
      enum: ['fit', 'native', 'zoom', 'pan', 'comparison', 'select-comparison', 'select-preview'],
    },
    maxItems: 7,
  });
  expect(source).toContain('| "select-comparison"');
  expect(source.split('\n').every((line) => line.length <= 100)).toBe(true);
  expect(() => rust({ ...array, items: { enum: ['a'.repeat(100)] } })).toThrow(
    'bounded compact validator',
  );
});
