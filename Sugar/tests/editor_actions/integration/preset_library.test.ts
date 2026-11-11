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
/** Safeguard persisted preset merging, queued saves and damaged-library protection. */
import { expect, test, jest } from '@jest/globals';
import { UserPresetLibrary } from '../../../frontend/comfyui/ui/editor_actions/application/UserPresetLibrary.js';
import { GLOBAL_ASSOCIATION } from '../../../frontend/comfyui/ui/editor_actions/domain/UserPreset.js';
import type { UserPreset } from '../../../frontend/comfyui/ui/editor_actions/domain/UserPreset.js';

test('merges dimension scopes, updates case-insensitive node names and serializes concurrent saves', async () => {
  let records: readonly UserPreset[] = [];
  const save = jest.fn(async (value: readonly UserPreset[]) => {
    records = value;
  });
  let next = 0;
  const library = new UserPresetLibrary(
    { load: async () => records, save },
    () => String(++next),
    () => 'now',
  );
  await library.prepare();
  await Promise.all([
    library.saveDimensions(960, 1344, GLOBAL_ASSOCIATION),
    library.saveDimensions(1344, 960, {
      scope: 'model_family',
      provider: 'civitai',
      key: 'sdxl',
      label: 'SDXL',
    }),
  ]);
  expect(records).toHaveLength(1);
  expect(records[0]?.associations).toHaveLength(2);
  await library.saveNode('Quality', 'KSampler', { steps: 28 }, GLOBAL_ASSOCIATION);
  await library.saveNode('quality', 'KSampler', { steps: 30 }, GLOBAL_ASSOCIATION);
  expect(records).toHaveLength(2);
  expect(library.nodePresets('KSampler', [GLOBAL_ASSOCIATION])[0]?.payload).toEqual({
    node_type: 'KSampler',
    inputs: { steps: 30 },
  });
});
test('failed writes retain the previous snapshot and damaged reads cannot be overwritten', async () => {
  const library = new UserPresetLibrary(
    {
      load: async () => [],
      save: async () => {
        throw new Error('disk');
      },
    },
    () => 'id',
    () => 'now',
  );
  await library.prepare();
  await expect(library.saveDimensions(10, 20, GLOBAL_ASSOCIATION)).rejects.toThrow('disk');
  expect(library.current()).toEqual([]);
  const save = jest.fn(async () => undefined);
  const damaged = new UserPresetLibrary(
    {
      load: async () => {
        throw new Error('damaged');
      },
      save,
    },
    () => 'id',
    () => 'now',
  );
  await expect(damaged.prepare()).rejects.toThrow('damaged');
  await expect(damaged.saveDimensions(10, 20, GLOBAL_ASSOCIATION)).rejects.toThrow('damaged');
  expect(save).not.toHaveBeenCalled();
});
