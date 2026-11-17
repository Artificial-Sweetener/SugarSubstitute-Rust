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
/** Prove the picker's wall can be mounted for an owner that keeps its own candidates. */
import { afterEach, beforeAll, expect, test } from '@jest/globals';
import { nextTick } from 'vue';
import { mountModelGallery } from '../../../frontend/comfyui/ui/model_library/vue/ModelGalleryVueRuntime.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { modelWallNeighbor } from '../../../frontend/comfyui/ui/model_library/ModelPickerNavigation.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

beforeAll(() => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
});
afterEach(() => document.body.replaceChildren());

test('shows the given models with one current tile and reports the tile a person chooses', async () => {
  const items = modelPickerChoices(
    [],
    'loras',
    Array.from({ length: 12 }, (_, index) => `lora-${String(index)}.safetensors`),
  );
  const element = document.createElement('div');
  document.body.append(element);
  const chosen: number[] = [];
  const gallery = mountModelGallery(element, { activate: (index) => chosen.push(index) });
  gallery.update(items, 3);
  await nextTick();

  const tiles = [...element.querySelectorAll<HTMLElement>('.sc-model-tile')];
  expect(tiles.map((tile) => tile.dataset.modelValue)).toEqual(items.map((item) => item.value));
  expect(element.querySelector('.sc-model-active')?.getAttribute('data-model-value')).toBe(
    'lora-3.safetensors',
  );
  // Nothing is the field's committed choice: the caller owns selection.
  expect(element.querySelector('.sc-model-current')).toBeNull();

  tiles[5]!.click();
  expect(chosen).toEqual([5]);

  // Arrow movement is the picker's own, over the wall as it is laid out.
  for (const key of ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'] as const)
    expect(gallery.neighbor(3, key)).toBe(modelWallNeighbor(items, 540, 3, key));

  gallery.update(items.slice(0, 2), 1);
  await nextTick();
  expect(element.querySelectorAll('.sc-model-tile')).toHaveLength(2);
  expect(element.querySelector('.sc-model-active')?.getAttribute('data-model-value')).toBe(
    'lora-1.safetensors',
  );

  gallery.dispose();
  expect(element.querySelector('.sc-model-tile')).toBeNull();
});
