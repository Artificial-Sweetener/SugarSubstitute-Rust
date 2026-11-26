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
/** Prove a card with no image says why, and a blurred image sharpens only under the pointer. */
import { afterEach, beforeAll, beforeEach, expect, test } from '@jest/globals';
import { nextTick } from 'vue';
import { mountModelGallery } from '../../../frontend/comfyui/ui/model_library/vue/ModelGalleryVueRuntime.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type {
  ModelCatalogItem,
  ModelImageState,
  ModelThumbnail,
} from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

beforeAll(() => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
});
beforeEach(() => imageDecoding.reset());
afterEach(() => document.body.replaceChildren());

/** One wall-sized prepared image by its asset identity. */
function thumbnail(digit: string): ModelThumbnail {
  const assetId = digit.repeat(64);
  return {
    assetId,
    role: 'standard',
    width: 256,
    height: 384,
    url: `/sugarcubes/v1/models/assets/${assetId}`,
  };
}

/** Catalog models whose cards are in the given states, in order. */
function models(states: readonly ModelImageState[]): ModelCatalogItem[] {
  return modelPickerChoices(
    [],
    'loras',
    states.map((state, index) => `${String(index)}-${state}.safetensors`),
  ).map((item, index) => ({
    ...item,
    image: { state: states[index] ?? 'none', blurred: false, reveal: [] },
  }));
}

/** Mount a wall over the given models. */
async function wall(items: readonly ModelCatalogItem[]) {
  const element = document.createElement('div');
  document.body.append(element);
  const gallery = mountModelGallery(element, { activate: () => undefined });
  gallery.update(items, 0);
  await nextTick();
  const tile = (index: number): HTMLElement => {
    const found = element.querySelectorAll<HTMLElement>('.sc-model-tile')[index];
    if (!found) throw new Error(`The wall has no tile ${String(index)}.`);
    return found;
  };
  return { element, gallery, tile };
}

test('each card without an image says why, in place of the image', async () => {
  const f = await wall(models(['pending', 'retrying', 'filtered', 'not_found', 'none']));
  const blank = (index: number): HTMLElement | null =>
    f.tile(index).querySelector('.sc-model-blank');

  // Waiting: the card flashes and says nothing.
  expect(blank(0)?.classList.contains('sc-model-blank-flash')).toBe(true);
  expect(blank(0)?.textContent).toBe('');
  // Waiting after a failure: it flashes and says so.
  expect(blank(1)?.classList.contains('sc-model-blank-flash')).toBe(true);
  expect(blank(1)?.textContent).toBe('Retrying…');
  // Excluded by the safety setting: an icon alone, explained by its tooltip.
  expect(blank(2)?.textContent).toBe('');
  expect(blank(2)?.getAttribute('title')).toBe('Hidden by your thumbnail safety setting');
  expect(blank(2)?.querySelector('[role="img"]')?.getAttribute('aria-label')).toBe(
    'Hidden by your thumbnail safety setting',
  );
  expect(blank(2)?.classList.contains('sc-model-blank-flash')).toBe(false);
  // No provider knows the file, and a known model with no image.
  expect(blank(3)?.textContent).toBe('Not Found');
  expect(blank(4)?.textContent).toBe('No Thumbnail');
  expect(f.element.textContent).not.toContain('No image');
  f.gallery.dispose();
});

test('a blurred image sharpens under the pointer and never under the keyboard', async () => {
  const [first, second] = models(['ready', 'ready']);
  if (!first || !second) throw new Error('The fixture has two models.');
  const blurred = thumbnail('b');
  const sharp = thumbnail('c');
  const items: ModelCatalogItem[] = [
    { ...first, thumbnails: [thumbnail('a')] },
    {
      ...second,
      thumbnails: [blurred],
      image: { state: 'ready', blurred: true, reveal: [sharp] },
    },
  ];
  const f = await wall(items);
  await imageDecoding.release();
  await nextTick();
  const shown = (): string[] =>
    [...f.tile(1).querySelectorAll('img')].map((image) => image.getAttribute('src') ?? '');
  expect(shown()).toEqual([blurred.url]);

  // Moving the wall's current model with the keyboard reveals nothing.
  f.gallery.update(items, 1);
  await nextTick();
  await imageDecoding.release();
  expect(shown()).toEqual([blurred.url]);

  f.tile(1).dispatchEvent(new Event('pointerenter'));
  await nextTick();
  await imageDecoding.release();
  await nextTick();
  expect(shown()).toContain(sharp.url);

  f.tile(1).dispatchEvent(new Event('pointerleave'));
  await nextTick();
  await imageDecoding.release();
  await nextTick();
  expect(shown().at(-1)).toBe(blurred.url);
  f.gallery.dispose();
});

test('a blurred image whose sharp files were not sent stays blurred under the pointer', async () => {
  const [only] = models(['ready']);
  if (!only) throw new Error('The fixture has one model.');
  const blurred = thumbnail('d');
  const f = await wall([
    { ...only, thumbnails: [blurred], image: { state: 'ready', blurred: true, reveal: [] } },
  ]);
  await imageDecoding.release();
  await nextTick();
  f.tile(0).dispatchEvent(new Event('pointerenter'));
  await nextTick();
  await imageDecoding.release();
  await nextTick();
  expect([...f.tile(0).querySelectorAll('img')].map((image) => image.getAttribute('src'))).toEqual([
    blurred.url,
  ]);
  f.gallery.dispose();
});
