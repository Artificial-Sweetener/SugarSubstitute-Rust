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
/** Prove named unions, string enums and flattened references compile to exact Rust decoders. */
import { rustContractBindings } from '../../../scripts/integrations/RustContractBindings.js';
import { rustContractDefinitions } from '../../../scripts/integrations/RustContractDefinitions.js';

type Schema = Record<string, unknown>;
const options = {
  schema: 'fixture.schema.json',
  generator: 'scripts/integrations/generate-fixture-contract.ts',
  prefix: 'Fixture',
};

/** Build a closed, fully required object from its members. */
function closed(properties: Schema): Schema {
  return {
    type: 'object',
    properties,
    required: Object.keys(properties),
    additionalProperties: false,
  };
}

/** Compile definitions exactly as authored. */
function compile(definitions: Schema): string {
  return rustContractBindings(definitions, JSON.stringify(definitions), options);
}

const flag = { type: 'boolean' };
const capabilities = {
  FixtureLayerCapabilities: closed({ display: flag }),
  FixtureGeometryCapabilities: closed({ display: flag, geometry: flag }),
  FixtureEditingCapabilities: closed({ display: flag, geometry: flag, editing: flag }),
  FixtureCapabilities: {
    oneOf: [
      { $ref: '#/$defs/FixtureLayerCapabilities' },
      { $ref: '#/$defs/FixtureGeometryCapabilities' },
      { $ref: '#/$defs/FixtureEditingCapabilities' },
    ],
  },
};

test('compiles disjoint named branches into an untagged enum with distinguishing variants', () => {
  const source = compile(capabilities);
  expect(source).toContain(
    '#[serde(untagged)]\npub(super) enum FixtureCapabilities {\n    Layer(FixtureLayerCapabilities),\n    Geometry(FixtureGeometryCapabilities),\n    Editing(FixtureEditingCapabilities),\n}',
  );
  expect(source).toContain('Self::Editing(value) => value.valid(),');
});

test.each([
  { ...capabilities, FixtureEditingCapabilities: closed({ display: flag, geometry: flag }) },
  {
    ...capabilities,
    FixtureEditingCapabilities: {
      ...closed({ display: flag, geometry: flag, editing: flag }),
      required: ['display', 'geometry'],
    },
  },
  {
    ...capabilities,
    FixtureCapabilities: { oneOf: [{ $ref: '#/$defs/FixtureLayerCapabilities' }] },
  },
  {
    ...capabilities,
    FixtureCapabilities: {
      oneOf: [{ $ref: '#/$defs/FixtureLayerCapabilities' }, { $ref: '#/$defs/FixtureMissing' }],
    },
  },
  {
    FixtureLayer: closed({ display: flag }),
    FixtureLayerCapabilities: closed({ editing: flag }),
    FixtureCapabilities: {
      oneOf: [{ $ref: '#/$defs/FixtureLayer' }, { $ref: '#/$defs/FixtureLayerCapabilities' }],
    },
  },
])('refuses overlapping, optional, single, unknown or indistinct untagged branches %#', (value) => {
  expect(() => compile(value)).toThrow();
});

test('compiles named kebab-case enums into Rust enums that need no further validation', () => {
  const source = compile({
    FixtureTool: { enum: ['pan-zoom', 'brush', 'shared-edges'] },
    FixtureState: closed({
      tool: { $ref: '#/$defs/FixtureTool' },
      tools: { type: 'array', items: { $ref: '#/$defs/FixtureTool' }, maxItems: 3 },
    }),
  });
  expect(source).toContain(
    '#[serde(rename_all = "kebab-case")]\npub(super) enum FixtureTool {\n    PanZoom,\n    Brush,\n    SharedEdges,\n}',
  );
  expect(source).toContain('pub(super) tool: FixtureTool,');
  expect(source).toContain('        self.tools.len() <= 3\n');
  expect(source).not.toContain('FixtureTool::valid');
});

test.each([
  { enum: [] },
  { enum: ['brush', 'brush'] },
  { enum: ['Brush'] },
  { enum: ['rgba8'] },
  { enum: ['-brush'] },
  { enum: ['brush'], description: 'extra' },
])('refuses enum tokens that would not round-trip through kebab-case %#', (node) => {
  expect(() => compile({ FixtureTool: node })).toThrow();
});

test('flattens referenced tagged unions and objects while keeping named untagged branches', () => {
  const query = {
    oneOf: [closed({ type: { const: 'context' } }), closed({ type: { const: 'capture' } })],
  };
  const definitions = {
    FixtureQuery: query,
    FixtureCommand: {
      oneOf: [
        { $ref: '#/$defs/FixtureQuery' },
        closed({ type: { const: 'watch' }, enabled: flag }),
      ],
    },
    FixtureContext: closed({ kind: { const: 'context' }, supported: flag }),
    FixtureResult: {
      oneOf: [{ $ref: '#/$defs/FixtureContext' }, closed({ kind: { const: 'failure' } })],
    },
    FixtureUnused: closed({ display: flag }),
    ...capabilities,
    FixtureRequest: closed({
      command: { $ref: '#/$defs/FixtureCommand' },
      result: { $ref: '#/$defs/FixtureResult' },
      capabilities: { $ref: '#/$defs/FixtureCapabilities' },
    }),
  };
  const selected = rustContractDefinitions(definitions, ['FixtureRequest']);
  expect(Object.keys(selected)).toEqual([
    'FixtureCommand',
    'FixtureResult',
    'FixtureLayerCapabilities',
    'FixtureGeometryCapabilities',
    'FixtureEditingCapabilities',
    'FixtureCapabilities',
    'FixtureRequest',
  ]);
  expect(selected.FixtureCommand).toEqual({
    oneOf: [...query.oneOf, closed({ type: { const: 'watch' }, enabled: flag })],
  });
  expect(selected.FixtureCapabilities).toEqual(capabilities.FixtureCapabilities);
  const source = compile(selected);
  expect(source).toContain('    #[serde(rename = "capture")]\n    Capture {},');
  expect(source).toContain('    #[serde(rename = "context")]\n    Context { supported: bool },');
});

test.each([
  [{ FixtureRoot: closed({ value: { $ref: '#/$defs/FixtureMissing' } }) }],
  [{ FixtureRoot: closed({ value: { $ref: 'other.json#/$defs/FixtureRoot' } }) }],
  [
    {
      FixtureRoot: closed({ value: { $ref: '#/$defs/FixtureLoop' } }),
      FixtureLoop: { oneOf: [{ $ref: '#/$defs/FixtureLoop' }, closed({ type: { const: 'a' } })] },
    },
  ],
])('refuses missing, external and cyclic references while selecting definitions %#', (value) => {
  expect(() => rustContractDefinitions(value, ['FixtureRoot'])).toThrow();
});
