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
/** Prove image-wall navigation follows its geometry and keeps model choices unchanged. */
import { modelWallNeighbor } from '../../../frontend/comfyui/ui/model_library/ModelPickerNavigation.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import {
  layoutModelWall,
  MODEL_DETECTOR_WALL_PROFILE,
} from '../../../frontend/comfyui/ui/model_library/ModelPickerLayout.js';

test('moves up/down between rows while left/right remain neighboring choices', () => {
  const items = modelPickerChoices(
    [],
    'loras',
    Array.from({ length: 18 }, (_, index) => `${String(index)}.safetensors`),
  );
  const rows = layoutModelWall(
    items.map(() => 0.72),
    540,
  );
  const current = rows[0]?.tiles[1]?.index;
  if (current === undefined) throw new Error('Fixture needs a multi-column row.');
  const down = modelWallNeighbor(items, 540, current, 'ArrowDown');
  expect(rows[1]?.tiles.some((tile) => tile.index === down)).toBe(true);
  expect(modelWallNeighbor(items, 540, down, 'ArrowUp')).toBe(current);
  expect(modelWallNeighbor(items, 540, current, 'ArrowRight')).toBe(current + 1);
  expect(modelWallNeighbor(items, 540, current, 'End')).toBe(17);
  expect(modelWallNeighbor([], 540, 0, 'ArrowDown')).toBe(0);
});

test('detector keyboard movement follows its own justified rows without committing a thumbnail', () => {
  const items = modelPickerChoices(
    [],
    'ultralytics',
    Array.from({ length: 17 }, (_, index) => `${String(index)}.pt`),
  );
  const rows = layoutModelWall(
    items.map(() => 0.72),
    700,
    MODEL_DETECTOR_WALL_PROFILE,
  );
  const current = rows[0]?.tiles[1]?.index;
  if (current === undefined) throw new Error('Missing detector row.');
  const down = modelWallNeighbor(items, 700, current, 'ArrowDown', MODEL_DETECTOR_WALL_PROFILE);
  expect(rows[1]?.tiles.some((tile) => tile.index === down)).toBe(true);
  expect(modelWallNeighbor(items, 700, down, 'ArrowUp', MODEL_DETECTOR_WALL_PROFILE)).toBe(current);
  expect(modelWallNeighbor(items, 700, current, 'Home', MODEL_DETECTOR_WALL_PROFILE)).toBe(0);
  expect(items[current]?.value).toBe(`${String(current)}.pt`);
});

test('wraps vertical movement between the first and last rows like the reference', () => {
  const items = modelPickerChoices(
    [],
    'loras',
    Array.from({ length: 12 }, (_, index) => `${String(index)}.safetensors`),
  );
  const rows = layoutModelWall(
    items.map(() => 0.72),
    560,
  );
  const first = rows[0]?.tiles[1]?.index;
  const last = rows.at(-1)?.tiles[1]?.index;
  if (first === undefined || last === undefined) throw new Error('Missing navigation rows.');
  expect(modelWallNeighbor(items, 560, first, 'ArrowUp')).toBe(last);
  expect(modelWallNeighbor(items, 560, last, 'ArrowDown')).toBe(first);
});
