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
/** Qualify omitted-or-present members as closed contracts in every generated language. */

import { runInNewContext } from 'node:vm';
import { ModuleKind, transpileModule } from 'typescript';
import { contractBindings } from '../../../scripts/integrations/ContractBindings.js';
import { rustContractBindings } from '../../../scripts/integrations/RustContractBindings.js';

type Schema = Record<string, unknown>;
const options = {
  schema: 'fixture.schema.json',
  generator: 'scripts/integrations/generate-fixture-contract.ts',
  prefix: 'Fixture',
};
const size: Schema = {
  type: 'object',
  properties: {
    width: { type: 'integer', minimum: 1, maximum: 10 },
    height: { type: 'integer', minimum: 1, maximum: 10 },
  },
  required: ['width', 'height'],
  additionalProperties: false,
};

/** Declare one required and one optional reference using the production object vocabulary. */
function definitions(state: Schema = {}): Schema {
  return {
    FixtureSize: size,
    FixtureState: {
      type: 'object',
      properties: {
        primary: { $ref: '#/$defs/FixtureSize' },
        secondary: { $ref: '#/$defs/FixtureSize' },
      },
      required: ['primary'],
      additionalProperties: false,
      ...state,
    },
  };
}

/** Generate portable TypeScript declarations and validators from a fixture schema. */
function typescript(state: Schema = {}): Promise<Readonly<Record<string, string>>> {
  return contractBindings(
    JSON.stringify({ $defs: definitions(state), $ref: '#/$defs/FixtureState' }),
    {
      ...options,
      union: 'FixtureMessage',
      types: 'types.ts',
      validation: 'validation.ts',
      importPath: './types.js',
      description: 'the optional-member fixture',
    },
  );
}

/** Generate Rust from raw schema nodes so invalid required lists remain observable. */
function rust(state: Schema = {}): string {
  const values = definitions(state);
  return rustContractBindings(values, JSON.stringify(values), options);
}

/** Execute the actual generated guard after erasing its compile-time-only imports. */
async function validator(): Promise<(value: unknown) => boolean> {
  const source = (await typescript())['validation.ts'];
  if (!source) throw new Error('Missing generated fixture validator.');
  const exports: Record<string, unknown> = {};
  runInNewContext(
    transpileModule(source, { compilerOptions: { module: ModuleKind.CommonJS } }).outputText,
    { exports },
  );
  const guard = exports.isFixtureMessage;
  if (typeof guard !== 'function') throw new Error('Missing fixture root guard.');
  return (value) => guard(value) === true;
}

test('declares an omitted member and admits it only as a valid present value', async () => {
  expect((await typescript())['types.ts']).toContain('readonly secondary?: FixtureSize }');
  const valid = await validator();
  const primary = { width: 2, height: 3 };
  expect(valid({ primary })).toBe(true);
  expect(valid({ primary, secondary: { width: 10, height: 1 } })).toBe(true);
  for (const value of [
    {},
    { secondary: primary },
    { primary, secondary: null },
    { primary, secondary: { width: 0, height: 1 } },
    { primary, secondary: { width: 1 } },
    { primary, secondary: { width: 1, height: 1, depth: 1 } },
    { primary, extra: true },
    { primary, secondary: primary, extra: true },
  ])
    expect(valid(value)).toBe(false);
});

test('emits a present-only Rust option that delegates to the referenced validator', () => {
  const source = rust();
  expect(source).toContain('pub(super) primary: FixtureSize,');
  expect(source).toContain('pub(super) secondary: Option<FixtureSize>,');
  expect(source).toContain('deserialize_with = "deserialize_present_option"');
  expect(source).toContain('skip_serializing_if = "Option::is_none"');
  expect(source).toContain('self.secondary.as_ref().is_none_or(FixtureSize::valid)');
  expect(source).toContain('T::deserialize(deserializer).map(Some)');
  expect(source).not.toContain('deserialize_required_option');
  const values = { FixtureSize: size };
  expect(rustContractBindings(values, JSON.stringify(values), options)).not.toContain(
    'deserialize_present_option',
  );
});

test('emits a present-only Rust option of a named string enum without a validator call', () => {
  const values = {
    ...definitions({
      properties: {
        primary: { $ref: '#/$defs/FixtureSize' },
        tone: { $ref: '#/$defs/FixtureTone' },
      },
    }),
    FixtureTone: { enum: ['dark', 'light'] },
  };
  const source = rustContractBindings(values, JSON.stringify(values), options);
  expect(source).toContain('pub(super) enum FixtureTone {');
  expect(source).toContain('pub(super) tone: Option<FixtureTone>,');
  expect(source).toContain('deserialize_with = "deserialize_present_option"');
  // Decoding already confines a string enum to its listed values; it has no valid() method.
  expect(source).not.toContain('FixtureTone::valid');
});

test('emits present-only Rust lists of a named definition and present-only booleans', () => {
  const source = rust({
    properties: {
      primary: { $ref: '#/$defs/FixtureSize' },
      sizes: { type: 'array', items: { $ref: '#/$defs/FixtureSize' }, maxItems: 4 },
      blocked: { type: 'boolean' },
    },
  });
  expect(source).toContain('pub(super) sizes: Option<Vec<FixtureSize>>,');
  expect(source).toContain('pub(super) blocked: Option<bool>,');
  expect(source).toContain('valid_fixture_state_sizes(self.sizes.as_deref())');
  expect(source).toContain(
    'fn valid_fixture_state_sizes(value: Option<&[FixtureSize]>) -> bool {\n' +
      '    value.is_none_or(|items| items.len() <= 4 && items.iter().all(FixtureSize::valid))\n}',
  );
  expect(source.match(/deserialize_with = "deserialize_present_option"/g)).toHaveLength(2);
});

test.each([
  { type: 'integer', minimum: 1, maximum: 10 },
  { type: 'boolean', default: false },
  { type: 'array', items: { type: 'integer', minimum: 1, maximum: 10 }, maxItems: 4 },
  { type: 'array', items: { $ref: '#/$defs/FixtureUnknown' }, maxItems: 4 },
])('refuses an optional Rust member outside the named, listed or boolean forms %#', (secondary) => {
  expect(() =>
    rust({ properties: { primary: { $ref: '#/$defs/FixtureSize' }, secondary } }),
  ).toThrow('optional members must reference a named definition, list one, or be boolean');
});

test.each([
  { required: ['secondary', 'primary'] },
  { required: ['primary', 'primary'] },
  { required: ['primary', 'unknown'] },
  { required: 'primary' },
  { additionalProperties: true },
])('refuses an ambiguous or open required list %#', async (state) => {
  expect(() => rust(state)).toThrow();
  await expect(typescript(state)).rejects.toThrow();
});
