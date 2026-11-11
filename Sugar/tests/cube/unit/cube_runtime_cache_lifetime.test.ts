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
/** Prove workflow-independent runtime caches survive repeated graph configuration. */
import { expect, jest, test } from '@jest/globals';
import { MemoryBudgetRegistry } from '../../../frontend/comfyui/ui/core/memory/MemoryBudgetRegistry.js';
import { CubeIconResolver } from '../../../frontend/comfyui/ui/core/CubeIconResolver.js';
import { ComfyCubeRuntimeCaches } from '../../../frontend/comfyui/ui/cube/ComfyCubeRuntimeCaches.js';
import { CubeCanvasPreviewImageCache } from '../../../frontend/comfyui/ui/surface/CubeCanvasPreviewImageCache.js';
import type { CubeVersionOption } from '../../../frontend/comfyui/ui/cube/version/CubeVersionTypes.js';

const option: CubeVersionOption = {
  value: '1.0.0',
  label: '1.0.0',
  revisionRef: 'v1',
  current: false,
  raw: null,
};

/** Answer exact artifact requests with a minimal valid import payload. */
function versionApi() {
  const loadRevision = jest.fn(async () => ({
    response: { ok: true, status: 200 } as Response,
    data: { cube_id: 'local/personal/Demo.cube', implementation: {}, version: '1.0.0' },
  }));
  return {
    listRevisions: jest.fn(async () => ({ response: { ok: true } as Response, data: {} })),
    load: loadRevision,
    loadRevision,
  };
}

/** Decode one image immediately so hosts observe ready icons and previews. */
function decodedImage(): HTMLImageElement {
  const image = document.createElement('img');
  Object.defineProperties(image, {
    naturalWidth: { value: 64 },
    naturalHeight: { value: 64 },
    complete: { value: true },
  });
  return image;
}

test('retains icons, previews and version artifacts across ten runtime replacements', async () => {
  const memory = new MemoryBudgetRegistry();
  const api = versionApi();
  const caches = new ComfyCubeRuntimeCaches(memory, api, console);
  const createImage = jest.fn(decodedImage);
  const icons: HTMLImageElement[] = [];
  const previews: (HTMLImageElement | null)[] = [];
  /** Build the graph-bound hosts a freshly configured runtime creates over the same caches. */
  const configure = (): void => {
    const resolver = new CubeIconResolver({
      images: caches.images.icons,
      imageFactory: createImage,
    });
    const entry = resolver.getImage(
      resolver.resolve({ cube_id: 'demo', icon: { kind: 'asset', url: '/icon.png' } }),
    );
    entry.image?.dispatchEvent(new Event('load'));
    if (entry.image) icons.push(entry.image);
    const preview = new CubeCanvasPreviewImageCache({
      images: caches.images.previews,
      createImage,
      invalidate: jest.fn(),
    });
    previews.push(preview.get('/view?filename=output.png', '4'));
  };

  for (let cycle = 0; cycle < 10; cycle++) {
    configure();
    await caches.versionRepository.loadArtifact('local/personal/Demo.cube', option);
  }

  expect(createImage).toHaveBeenCalledTimes(2);
  expect(new Set(icons).size).toBe(1);
  expect(new Set(previews).size).toBe(1);
  expect(previews[0]).not.toBeNull();
  expect(api.loadRevision).toHaveBeenCalledTimes(1);
  expect(memory.usage().consumers.map((consumer) => consumer.id)).toEqual([
    'cube-icons',
    'cube-preview-images',
    'cube-version-artifacts',
  ]);

  caches.invalidateLibrary();
  await caches.versionRepository.loadArtifact('local/personal/Demo.cube', option);
  expect(api.loadRevision).toHaveBeenCalledTimes(2);
});
