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
/** Prove what a compiled project script hands the page, and what the page keeps for its writer. */
import { expect, test } from '@jest/globals';
import {
  carryScriptComposition,
  markUnsavedOutputs,
  readImportedScriptRelations,
  readUnsavedOutputInstances,
} from '../../../frontend/comfyui/ui/workflow/ImportedScriptState.js';

const endpoint = (instance: string, symbol: string, input: string) => ({
  instance_id: instance,
  node_symbol: symbol,
  input_name: input,
});
const field = (instance: string, symbol: string, input: string) =>
  JSON.stringify([JSON.stringify(['cube', instance, symbol]), input]);

const compiled = {
  nodes: [
    { properties: { sugarcubes_cube: { instance_id: 'first' } } },
    { properties: { sugarcubes_cube: { instance_id: 'second' }, sugarcubes_save_outputs: false } },
  ],
  extra: {
    sugarcubes_composition: {
      schema_version: 1,
      field_annotations: [{ namespace: 'substitute.model_asset', payload: { sha256: 'AB' } }],
      value_relations: [
        {
          kind: 'global_override',
          key: 'seed',
          value: 7,
          targets: [endpoint('first', 'ksampler', 'seed'), endpoint('second', 'ksampler', 'seed')],
        },
        { kind: 'global_override', key: 'unreadable', value: { nested: true }, targets: [] },
        {
          kind: 'field_link',
          source: endpoint('first', 'prompt', 'value'),
          targets: [endpoint('second', 'prompt', 'value')],
          own_value: 'kept text',
        },
        {
          kind: 'field_link',
          source: endpoint('first', 'negative', 'value'),
          targets: [endpoint('second', 'negative', 'value')],
        },
      ],
      script_state: {
        fixed_seed_overrides: ['seed', 4],
        fixed_seed_fields: [endpoint('first', 'ksampler', 'seed'), 'unreadable'],
        authored_fields: [endpoint('first', 'ksampler', 'cfg')],
      },
    },
  },
};

test('a compiled script hands over its overrides, own values and fixed seeds', () => {
  expect(readImportedScriptRelations(compiled)).toEqual({
    overrides: [{ key: 'seed', value: 7 }],
    fixedSeedOverrides: ['seed'],
    fixedSeedFields: [field('first', 'ksampler', 'seed')],
    ownValues: [{ field: field('second', 'prompt', 'value'), value: 'kept text' }],
  });
  expect(readImportedScriptRelations({})).toEqual({
    overrides: [],
    fixedSeedOverrides: [],
    fixedSeedFields: [],
    ownValues: [],
  });
});

test('the workflow keeps what the script writer reads beside the page own records', () => {
  const exact = { kind: 'exact_integer', digits: '9007199254740993', targets: [] };
  const root = {
    extra: { other: 1, sugarcubes_composition: { schema_version: 1, value_relations: [exact] } },
  };
  carryScriptComposition(root, compiled);
  expect(root.extra).toEqual({
    other: 1,
    sugarcubes_composition: {
      schema_version: 1,
      value_relations: [exact],
      field_annotations: compiled.extra.sugarcubes_composition.field_annotations,
      script_state: compiled.extra.sugarcubes_composition.script_state,
    },
  });
  const untouched = { extra: { other: 1 } };
  carryScriptComposition(untouched, { extra: {} });
  expect(untouched).toEqual({ extra: { other: 1 } });
});

test('Cubes a script says are not saved are marked so on the workflow', () => {
  expect(readUnsavedOutputInstances(compiled)).toEqual(['second']);
  const first = {
    properties: { sugarcubes_cube: { instance_id: 'first' } } as Record<string, unknown>,
  };
  const second = {
    properties: { sugarcubes_cube: { instance_id: 'second' } } as Record<string, unknown>,
  };
  markUnsavedOutputs({ _nodes: [first, second] }, ['second']);
  expect(first.properties.sugarcubes_save_outputs).toBeUndefined();
  expect(second.properties.sugarcubes_save_outputs).toBe(false);
});
