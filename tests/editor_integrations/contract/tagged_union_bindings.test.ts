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
/** Prove explicit tagged unions remain closed and reject unsupported schema shapes. */
import { rustContractBindings } from '../../../scripts/integrations/RustContractBindings.js';

type Schema = Record<string, unknown>;
const options = {
  schema: 'fixture.schema.json',
  generator: 'scripts/integrations/generate-fixture-contract.ts',
  prefix: 'Fixture',
};

/** Exercise the same empty and numeric command variants used by viewport requests. */
function branch(tag: string, fields: Schema = {}): Schema {
  const properties = { type: { const: tag }, ...fields };
  return {
    type: 'object',
    properties,
    required: Object.keys(properties),
    additionalProperties: false,
  };
}

/** Compile arbitrary authored nodes without JSON normalization hiding invalid shapes. */
function compile(node: Schema): string {
  const definitions = { FixtureCommand: node };
  return rustContractBindings(definitions, JSON.stringify(definitions), options);
}

test('emits internally tagged closed struct variants, numeric validation and camel-case fields', () => {
  const source = compile({
    oneOf: [
      branch('read'),
      branch('zoom', { zoomFactor: { type: 'number', minimum: 0.01, maximum: 100 } }),
    ],
  });
  expect(source).toContain('tag = "type", deny_unknown_fields, rename_all_fields = "camelCase"');
  expect(source).toContain('Read {}');
  expect(source).toContain('Zoom { zoom_factor: f64 }');
  expect(source).toContain('(0.01..=100.0).contains(zoom_factor)');
  expect(source).not.toContain('serde(untagged)');
  expect(source).not.toContain('deserialize_required_option');
});

test('validates numeric constants without string coercion and ignores intrinsically typed members', () => {
  const source = compile({
    oneOf: [
      branch('read'),
      branch('configure', { schema: { const: 1 }, enabled: { type: 'boolean' } }),
    ],
  });
  expect(source).toContain('schema, enabled: _');
  expect(source).toContain('(*schema) == 1');
  expect(source).not.toContain('schema.as_str()');
});

test('keeps branch-local validators distinct when variants reuse a field name', () => {
  const source = compile({
    oneOf: [
      branch('first', { selection: { enum: ['alpha-first', 'alpha-second', 'alpha-third'] } }),
      branch('second', { selection: { enum: ['other-first', 'other-second', 'other-third'] } }),
    ],
  });
  expect(source).toContain('fn valid_fixture_command_first_selection(');
  expect(source).toContain('fn valid_fixture_command_second_selection(');
});

test.each([
  { oneOf: [] },
  { oneOf: [branch('read')] },
  { oneOf: Array.from({ length: 17 }, (_, index) => branch(`tag-${String(index)}`)) },
  { oneOf: [branch('read'), branch('read')] },
  { oneOf: [branch('read'), branch('unsafe_tag')] },
  { oneOf: [branch('read'), branch('self')] },
  { oneOf: [branch('read'), branch('x'.repeat(65))] },
  { oneOf: [branch('read'), { ...branch('fit'), additionalProperties: true }] },
  { oneOf: [branch('read'), { ...branch('fit'), required: [] }] },
  { oneOf: [branch('read'), { ...branch('fit'), unknownKeyword: true }] },
  { oneOf: [branch('read'), { $ref: '#/$defs/FixtureCommand' }] },
  { oneOf: [branch('read'), branch('fit', { type: { enum: ['fit'] } })] },
  { oneOf: [branch('read'), null] },
  { oneOf: [branch('read'), branch('fit')], additionalProperties: false },
  { oneOf: [branch('read'), branch('fit', { payload: { type: 'object' } })] },
  {
    oneOf: [
      branch('read'),
      branch('fit', {
        color: {
          oneOf: [{ type: 'string', maxLength: 7, pattern: '^#[a-fA-F0-9]{6}$' }, { const: null }],
        },
      }),
    ],
  },
])('rejects unsupported or ambiguous tagged schema %#', (node) => {
  expect(() => compile(node)).toThrow();
});
