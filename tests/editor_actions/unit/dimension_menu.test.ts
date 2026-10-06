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
/** Compare menu hierarchy and actions with the Substitute dimension menu contract. */
import { expect, test } from '@jest/globals';
import { dimensionMenuEntries } from '../../../frontend/comfyui/ui/editor_actions/presentation/DimensionMenu.js';
import { UserPresetLibrary } from '../../../frontend/comfyui/ui/editor_actions/application/UserPresetLibrary.js';
import { GLOBAL_ASSOCIATION } from '../../../frontend/comfyui/ui/editor_actions/domain/UserPreset.js';

test('clicked side chooses its ratio anchor; title menus offer both and preset orientations', async () => {
  const library = new UserPresetLibrary(
    { load: async () => [], save: async () => undefined },
    () => 'id',
    () => 'now',
  );
  await library.prepare();
  await library.saveDimensions(960, 1344, GLOBAL_ASSOCIATION);
  let width = 1024,
    height = 1024;
  const binding = {
    width: {
      read: () => width,
      write: (value: number) => {
        width = value;
      },
      disabled: () => false,
    },
    height: {
      read: () => height,
      write: (value: number) => {
        height = value;
      },
      disabled: () => false,
    },
  };
  const entries = dimensionMenuEntries(binding, 'width', library, [], () => undefined);
  expect(entries.map((entry) => entry.label)).toEqual([
    'Swap width & height',
    'Set dimensions',
    'Set ratio by Width',
    '',
    'Save current dimensions',
  ]);
  expect(entries[2]?.children?.map((entry) => entry.label)).toEqual(['Landscape', 'Portrait']);
  entries[2]?.children?.[0]?.children?.find((entry) => entry.label === '16:9')?.select?.();
  expect([width, height]).toEqual([1024, 576]);
  entries[1]?.children?.[1]?.children?.[1]?.select?.();
  expect([width, height]).toEqual([1344, 960]);
  expect(
    dimensionMenuEntries(binding, null, library, [], () => undefined).map((entry) => entry.label),
  ).toContain('Set ratio by Height');
});
