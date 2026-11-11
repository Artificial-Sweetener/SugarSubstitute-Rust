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
/** Prove source preferences and output captions stay literal, portable and instance scoped. */
import {
  decodeSourceSavingPreferences,
  encodeSourceSavingPreferences,
  readWorkflowSourceSavingPreferences,
} from '../../../frontend/comfyui/ui/output_saving/domain/SourceSavingPreferences.js';
import { decodeComfyWorkflowSources } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowSourceDecoder.js';

test('keeps two instances of a shared saver independent and preserves literal captions', () => {
  const sources = {
    'a:save': { enabled: false, label: '<first instance>' },
    'b:save': { enabled: true, label: 'Second instance' },
  };
  const preference = { schema_version: 1, sources };
  const workflow = {
    nodes: [
      { id: 'a', type: 'shared', title: 'First' },
      { id: 'b', type: 'shared', title: 'Second' },
    ],
    definitions: {
      subgraphs: [
        { id: 'shared', nodes: [{ id: 'save', type: 'SaveImage', title: 'Shared save' }] },
      ],
    },
    extra: { sugarcubes_output_saving: { source_preferences: preference } },
  };
  expect(
    decodeComfyWorkflowSources(workflow)
      .filter((source) => source.executionPath.includes(':'))
      .map((source) => [source.executionPath, source.label]),
  ).toEqual([
    ['a:save', '<first instance>'],
    ['b:save', 'Second instance'],
  ]);
  const parsed = readWorkflowSourceSavingPreferences(workflow);
  expect(parsed.get('a:save')?.enabled).toBe(false);
  expect(encodeSourceSavingPreferences(parsed)).toEqual(preference);
  expect(workflow.definitions.subgraphs[0]?.nodes[0]?.title).toBe('Shared save');
});

test.each([
  [],
  {},
  { schema_version: true, sources: {} },
  { schema_version: 2, sources: {} },
  { schema_version: 1, sources: [] },
  { schema_version: 1, sources: { save: { enabled: 'yes' } } },
  { schema_version: 1, sources: { save: { label: [] } } },
])('rejects corrupt explicit source preferences: %j', (value) =>
  expect(() => decodeSourceSavingPreferences(value)).toThrow(),
);
