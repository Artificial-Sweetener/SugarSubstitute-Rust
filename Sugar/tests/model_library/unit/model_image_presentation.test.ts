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
/** The one way a model image is shown, on every surface that shows one. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { createApp, h, nextTick, reactive } from 'vue';
import type { ModelThumbnail } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import {
  DECODED_MODEL_IMAGES,
  ModelImagePresentation,
  decodeModelImage,
  forgetDecodedModelImages,
  modelImageDecoded,
} from '../../../frontend/comfyui/ui/model_library/ModelImagePresentation.js';
import { ModelCatalogImage } from '../../../frontend/comfyui/ui/model_library/vue/ModelCatalogImage.js';
import { ModelRecommendationPreview } from '../../../frontend/comfyui/ui/model_library/vue/ModelRecommendationPreview.js';

afterEach(() => {
  document.body.replaceChildren();
  imageDecoding.reset();
  forgetDecodedModelImages();
  jest.restoreAllMocks();
});

const settled = async (): Promise<void> => {
  await imageDecoding.release();
  await nextTick();
};

test('the image on screen changes only when its replacement has been decoded', async () => {
  const changed = jest.fn();
  const image = new ModelImagePresentation(changed);
  imageDecoding.held = true;
  image.show('/a.webp');
  expect([image.shown, image.loading, image.unavailable]).toEqual([null, true, false]);
  await imageDecoding.release();
  expect([image.shown, image.loading]).toEqual(['/a.webp', false]);
  // A replacement decodes out of sight; the first image stays until it is ready.
  image.show('/b.webp');
  expect([image.shown, image.loading]).toEqual(['/a.webp', false]);
  // A newer replacement supersedes the one still decoding, and neither blanks the image.
  image.show('/c.webp');
  await imageDecoding.release('/b.webp');
  expect(image.shown).toBe('/a.webp');
  await imageDecoding.release('/c.webp');
  expect(image.shown).toBe('/c.webp');
  expect(changed).toHaveBeenCalled();
});

test('a replacement that fails leaves the image on screen and is tried again only from a new record', async () => {
  const image = new ModelImagePresentation(() => undefined);
  const decode = jest.spyOn(HTMLImageElement.prototype, 'decode');
  const record = { url: '/broken.webp' };
  image.show('/a.webp');
  await imageDecoding.release();
  imageDecoding.broken.add('/broken.webp');
  image.show(record.url, record);
  await imageDecoding.release();
  expect([image.shown, image.unavailable]).toEqual(['/a.webp', false]);
  const attempts = decode.mock.calls.length;
  // The same record asking again starts nothing.
  image.show(record.url, record);
  await imageDecoding.release();
  expect(decode).toHaveBeenCalledTimes(attempts);
  // The catalog publishing the model's image again, a new record, tries the address again.
  imageDecoding.broken.clear();
  image.show(record.url, { url: record.url });
  await imageDecoding.release();
  expect(image.shown).toBe('/broken.webp');
  // With nothing on screen a failure is unavailable.
  const empty = new ModelImagePresentation(() => undefined);
  imageDecoding.broken.add('/missing.webp');
  empty.show('/missing.webp');
  await imageDecoding.release();
  expect([empty.shown, empty.loading, empty.unavailable]).toEqual([null, false, true]);
});

test('a decoded image is kept by address and shown at once wherever it is asked for again', async () => {
  const decode = jest.spyOn(HTMLImageElement.prototype, 'decode');
  expect(await decodeModelImage('/kept.webp')).toBe(true);
  expect(modelImageDecoded('/kept.webp')).toBe(true);
  expect(decode).toHaveBeenCalledTimes(1);
  // Another surface: no decode, no loading state, the image at once.
  const image = new ModelImagePresentation(() => undefined);
  image.show('/kept.webp');
  expect([image.shown, image.loading]).toEqual(['/kept.webp', false]);
  expect(decode).toHaveBeenCalledTimes(1);
  // Surfaces asking for one address together share one decode.
  imageDecoding.held = true;
  const first = decodeModelImage('/shared.webp');
  const second = decodeModelImage('/shared.webp');
  expect(second).toBe(first);
  await imageDecoding.release();
  // The oldest kept images go first once more are kept than the limit.
  imageDecoding.held = false;
  for (let index = 0; index <= DECODED_MODEL_IMAGES; index++)
    await decodeModelImage(`/many-${String(index)}.webp`);
  expect(modelImageDecoded('/kept.webp')).toBe(false);
  expect(modelImageDecoded(`/many-${String(DECODED_MODEL_IMAGES)}.webp`)).toBe(true);
});

test('a catalog image keeps its element through a replacement and falls back only with nothing to show', async () => {
  const state = reactive<{ image: ModelThumbnail }>({ image: thumbnail('/a.webp') });
  const root = document.createElement('div');
  document.body.append(root);
  const app = createApp({
    render: () => h(ModelCatalogImage, { image: state.image, fallback: true, class: 'banner' }),
  });
  app.mount(root);
  imageDecoding.held = true;
  expect(root.querySelector('img')).toBeNull();
  await settled();
  const first = root.querySelector('img');
  expect(first?.getAttribute('src')).toBe('/a.webp');
  expect(first?.className).toBe('banner');
  state.image = thumbnail('/b.webp');
  await nextTick();
  // Still the first image, the same element, while the replacement decodes.
  expect(root.querySelector('img')).toBe(first);
  expect(first?.getAttribute('src')).toBe('/a.webp');
  await settled();
  expect(root.querySelector('img')).toBe(first);
  expect(first?.getAttribute('src')).toBe('/b.webp');
  imageDecoding.broken.add('/c.webp');
  state.image = thumbnail('/c.webp');
  await nextTick();
  await settled();
  expect(first?.getAttribute('src')).toBe('/b.webp');
  expect(root.querySelector('.sc-model-no-image')).toBeNull();
  app.unmount();
  // With nothing shown yet, a broken image is the fallback.
  const empty = document.createElement('div');
  document.body.append(empty);
  const second = createApp({
    render: () => h(ModelCatalogImage, { image: thumbnail('/c.webp'), fallback: true }),
  });
  second.mount(empty);
  await settled();
  expect(empty.querySelector('.sc-model-no-image')?.textContent).toBe('No image');
  second.unmount();
});

test('a portrait shows loading only before its first image and keeps that image through a change', async () => {
  const state = reactive<{ url: string | null }>({ url: '/one.webp' });
  const root = document.createElement('div');
  document.body.append(root);
  imageDecoding.held = true;
  const app = createApp({ render: () => h(ModelRecommendationPreview, { url: state.url }) });
  app.mount(root);
  expect(root.querySelector('.sc-model-preview-loading')).not.toBeNull();
  expect(root.querySelector('img')).toBeNull();
  await settled();
  expect(root.querySelector('.sc-model-preview-loading')).toBeNull();
  expect(root.querySelector('img')?.getAttribute('src')).toBe('/one.webp');
  state.url = '/two.webp';
  await nextTick();
  expect(root.querySelector('img')?.getAttribute('src')).toBe('/one.webp');
  expect(root.querySelector('.sc-model-preview-loading')).toBeNull();
  await settled();
  expect(root.querySelector('img')?.getAttribute('src')).toBe('/two.webp');
  state.url = null;
  await nextTick();
  expect(root.querySelector('img')).toBeNull();
  expect(root.querySelector('.sc-model-preview-unavailable')?.textContent).toBe(
    'Preview unavailable',
  );
  app.unmount();
});

function thumbnail(url: string): ModelThumbnail {
  return { role: 'banner', url, width: 768, height: 256 } as ModelThumbnail;
}
