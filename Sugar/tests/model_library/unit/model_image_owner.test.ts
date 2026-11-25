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
/**
 * Model images have one owner: one rule for which image a surface shows, one decoded image per
 * address shared by every prompt, and a catalog publication that costs a wall and the nodes
 * nothing when no file and no tile shape changed.
 */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  MODEL_BANNER_SIZE,
  chooseModelThumbnail,
  type ModelCatalogItem,
  type ModelCatalogSnapshot,
  type ModelThumbnail,
} from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import { layoutModelGallery } from '../../../frontend/comfyui/ui/model_library/ModelGalleryLayout.js';
import { sameModelFiles } from '../../../frontend/comfyui/ui/model_library/ModelPickerHost.js';
import { modelVersionImage } from '../../../frontend/comfyui/ui/model_library/ModelReleases.js';
import {
  IDLE_LORA_RASTERS,
  decodeLoraMedia,
  forgetLoraRasters,
} from '../../../frontend/sugarprompt/editor/src/lora-media-decoder.js';

afterEach(() => {
  forgetLoraRasters();
  imageDecoding.reset();
  jest.restoreAllMocks();
});

const image = (role: 'standard' | 'banner', url: string, width: number, height: number) =>
  ({ assetId: url, role, url, width, height }) as ModelThumbnail;

const model = (id: string, thumbnails: ModelThumbnail[] = []): ModelCatalogItem =>
  ({
    id,
    kind: 'loras',
    value: `${id}.safetensors`,
    title: id,
    providerVersionId: 'v1',
    thumbnails,
  }) as unknown as ModelCatalogItem;

test('one rule chooses a model’s picture for a field, a prompt chip and a version', () => {
  const item = model('a', [
    image('banner', '/banner-384', 384, 80),
    image('banner', '/banner-768', 768, 160),
    image('standard', '/standard-512', 512, 512),
  ]);
  // The smallest image of the wanted role that is large enough for its destination.
  expect(chooseModelThumbnail(item, 'banner', MODEL_BANNER_SIZE)?.url).toBe('/banner-768');
  expect(chooseModelThumbnail(item, 'banner', 300)?.url).toBe('/banner-384');
  // The version in use is the installed model: its prepared image. Any other has its own preview.
  const version = (versionId: string, previewUrl: string | null, inUse: boolean | null = null) => ({
    versionId,
    previewUrl,
    inUse,
  });
  expect(modelVersionImage(item, version('v1', '/provider-v1'), 'standard', 512)).toBe(
    '/standard-512',
  );
  expect(modelVersionImage(item, version('v2', '/provider-v2'), 'standard', 512)).toBe(
    '/provider-v2',
  );
  expect(modelVersionImage(item, version('v2', '/provider-v2', true), 'banner', 768)).toBe(
    '/banner-768',
  );
  // A version with no preview of its own falls back to the model's image, and then to none.
  expect(modelVersionImage(item, version('v3', null), 'banner', 768)).toBe('/banner-768');
  expect(modelVersionImage(model('bare'), version('v3', null), 'banner', 768)).toBeNull();
});

test('a wall is laid out again only when a tile’s shape, the width or the profile changes', () => {
  const items = Array.from({ length: 40 }, (_, index) =>
    model(`m${String(index)}`, [image('standard', `/s${String(index)}`, 512, 640 + index)]),
  );
  const rows = layoutModelGallery(items, 540);
  // The catalog republished: new array, the same models, one of them with a new name and picture.
  const republished = items.map((item, index) =>
    index === 3
      ? ({
          ...item,
          title: 'Renamed',
          thumbnails: [image('standard', '/new-picture', 512, 643)],
        } as ModelCatalogItem)
      : item,
  );
  expect(layoutModelGallery(republished, 540)).toBe(rows);
  // A picture of another shape moves its neighbours, so the wall is laid out again.
  const reshaped = items.map((item, index) =>
    index === 3
      ? ({ ...item, thumbnails: [image('standard', '/wide', 1024, 512)] } as ModelCatalogItem)
      : item,
  );
  const next = layoutModelGallery(reshaped, 540);
  expect(next).not.toBe(rows);
  expect(layoutModelGallery(reshaped, 720)).not.toBe(next);
});

test('a publication with the same model files is not a reason to read the nodes again', () => {
  const snapshot = (items: ModelCatalogItem[], scanning = false): ModelCatalogSnapshot =>
    ({ revision: 1, scanning, items }) as unknown as ModelCatalogSnapshot;
  const a = model('a');
  const b = model('b');
  expect(sameModelFiles(snapshot([a, b]), snapshot([a, { ...b, title: 'New name' }]))).toBe(true);
  expect(sameModelFiles(snapshot([a, b]), snapshot([a, b, model('c')]))).toBe(false);
  expect(sameModelFiles(snapshot([a, b]), snapshot([b, a]))).toBe(false);
  expect(sameModelFiles(snapshot([a, b]), snapshot([a, b], true))).toBe(false);
});

test('every prompt shares one decoded image per address, kept after its chips have gone', async () => {
  const closed: string[] = [];
  const bitmaps = jest.fn((source: HTMLImageElement) =>
    Promise.resolve({
      width: 64,
      height: 64,
      close: () => closed.push(source.getAttribute('src') ?? 'released'),
    }),
  );
  Object.defineProperty(globalThis, 'createImageBitmap', { value: bitmaps, configurable: true });
  jest.spyOn(HTMLImageElement.prototype, 'naturalWidth', 'get').mockReturnValue(768);
  jest.spyOn(HTMLImageElement.prototype, 'naturalHeight', 'get').mockReturnValue(160);
  const reference = (url: string) => ({ status: 'ready' as const, url, width: 768, height: 160 });
  const signal = new AbortController().signal;
  imageDecoding.held = true;
  // Two prompts ask for the same banner together: one decode.
  const first = decodeLoraMedia(reference('/lora-a'), signal);
  const second = decodeLoraMedia(reference('/lora-a'), signal);
  await imageDecoding.release();
  const [one, two] = await Promise.all([first, second]);
  expect(bitmaps).toHaveBeenCalledTimes(1);
  expect(one && two && one !== two).toBe(true);
  // Both let go; a third prompt later still decodes nothing.
  one?.dispose();
  two?.dispose();
  imageDecoding.held = false;
  const later = await decodeLoraMedia(reference('/lora-a'), signal);
  expect(bitmaps).toHaveBeenCalledTimes(1);
  expect(closed).toEqual([]);
  // A waiter cancelled before the decode finished takes no hold; the image stays for the next.
  const cancelled = new AbortController();
  imageDecoding.held = true;
  const dropped = decodeLoraMedia(reference('/lora-b'), cancelled.signal);
  cancelled.abort();
  await imageDecoding.release();
  expect(await dropped).toBeNull();
  imageDecoding.held = false;
  expect(await decodeLoraMedia(reference('/lora-b'), signal)).not.toBeNull();
  expect(bitmaps).toHaveBeenCalledTimes(2);
  // An image in use is never closed, however many idle ones come and go.
  for (let index = 0; index <= IDLE_LORA_RASTERS; index++)
    (await decodeLoraMedia(reference(`/idle-${String(index)}`), signal))?.dispose();
  expect(closed).toHaveLength(1);
  expect(later).not.toBeNull();
  expect(closed).not.toContain('/lora-a');
});
