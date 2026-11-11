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
/** Prove byte-shape compatibility with Substitute's version-one preset document. */
import { expect, test } from '@jest/globals';
import {
  decodePresetDocument,
  encodePresetDocument,
} from '../../../frontend/comfyui/ui/editor_actions/domain/PresetCodec.js';

const documentValue = {
  version: 1,
  presets: [
    {
      id: 'dimension:abc',
      kind: 'dimension',
      label: '960 x 1344',
      payload: { short_edge: 960, long_edge: 1344 },
      associations: [
        { scope: 'model_family', provider: 'civitai', key: 'illustrious', label: 'Illustrious' },
        { scope: 'global', provider: null, key: 'global', label: 'Global' },
      ],
      created_at: '2026-10-04T00:00:00Z',
      updated_at: '2026-10-04T00:00:00Z',
    },
    {
      id: 'node_inputs:def',
      kind: 'node_inputs',
      label: '<b>Quality</b>',
      payload: { node_type: 'KSampler', inputs: { steps: 28, cfg: 5.5, enabled: true } },
      associations: [
        { scope: 'provider_model_version', provider: 'civitai', key: '123', label: 'Version' },
      ],
      created_at: '2026-10-04T00:00:00Z',
      updated_at: '2026-10-04T00:00:00Z',
    },
    {
      id: 'prompt_string:xyz',
      kind: 'prompt_string',
      label: 'Untouched prompt library',
      payload: { text: 'literal <tag>' },
      associations: [],
      created_at: 'a',
      updated_at: 'b',
    },
  ],
};
test('round trips identities, associations, timestamps and excluded prompt data losslessly', () => {
  expect(encodePresetDocument(decodePresetDocument(documentValue))).toEqual(documentValue);
});
test('rejects wrong versions and unsafe node-input data before persistence', () => {
  expect(() => decodePresetDocument({ ...documentValue, version: 2 })).toThrow();
  const node = documentValue.presets[1];
  expect(() =>
    decodePresetDocument({
      version: 1,
      presets: [{ ...node, payload: { node_type: 'KSampler', inputs: { model: ['12', 0] } } }],
    }),
  ).toThrow();
  expect(() =>
    decodePresetDocument({
      version: 1,
      presets: [{ ...documentValue.presets[0], payload: { short_edge: 1344, long_edge: 960 } }],
    }),
  ).toThrow();
});
