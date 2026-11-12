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
/** Safeguard side-effect-free prepared catalog access for editor menu scope projection. */
import { expect, test } from '@jest/globals';
import { ModelCatalogClient } from '../../../frontend/comfyui/ui/model_library/ModelCatalogClient.js';
import { decodeModelCatalog } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';

test.each([undefined, null, 0, 1, 2])('validates prepared version count %s', (versionCount) => {
  const item = modelPickerChoices([], 'checkpoints', ['fixture.safetensors'])[0];
  if (!item) throw new Error('Missing catalog fixture.');
  const snapshot = decodeModelCatalog({
    schemaVersion: 1,
    revision: 1,
    models: [{ ...item, versionCount }],
    fieldContracts: [],
  });
  expect(snapshot.items[0]?.versionCount).toBe(versionCount ?? null);
});

test.each([-1, 1.5, '1', true, {}])(
  'rejects malformed prepared version count %s',
  (versionCount) => {
    const item = modelPickerChoices([], 'checkpoints', ['fixture.safetensors'])[0];
    expect(() =>
      decodeModelCatalog({
        schemaVersion: 1,
        revision: 1,
        models: [{ ...item, versionCount }],
        fieldContracts: [],
      }),
    ).toThrow('Invalid model version count.');
  },
);
test('reads only prepared snapshots without initiating IO from menu consumers', async () => {
  let calls = 0;
  const client = new ModelCatalogClient(async () => {
    calls += 1;
    // The client consumes only status and JSON at this isolated external transport boundary.
    return {
      ok: true,
      status: 200,
      json: async () => ({ schemaVersion: 1, revision: 1, models: [], fieldContracts: [] }),
    } as Response;
  });
  expect(client.current()).toBeNull();
  expect(calls).toBe(0);
  const loaded = await client.load();
  expect(client.current()).toBe(loaded);
  expect(calls).toBe(1);
});
