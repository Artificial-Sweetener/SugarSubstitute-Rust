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
/** Verify the Nodes 1.0 Cube preview image lifecycle. */

import { jest } from '@jest/globals';

import { CubeCanvasPreviewImageCache } from '../../../frontend/comfyui/ui/surface/CubeCanvasPreviewImageCache.js';

describe('CubeCanvasPreviewImageCache', () => {
  test('reuses an already-decoded Comfy image without loading a duplicate', () => {
    const loaded = document.createElement('img');
    Object.defineProperties(loaded, {
      naturalWidth: { value: 1440 },
      naturalHeight: { value: 2016 },
    });
    const createImage = jest.fn<() => HTMLImageElement>();
    const cache = new CubeCanvasPreviewImageCache({
      createImage,
      findLoadedImage: (url) => (url === '/view?filename=output.png' ? loaded : null),
      invalidate: jest.fn(),
    });

    expect(cache.get('/view?filename=output.png')).toBe(loaded);
    expect(createImage).not.toHaveBeenCalled();
  });

  test('reports a failed image and permits a later retry', () => {
    const first = document.createElement('img');
    const second = document.createElement('img');
    const createImage = jest
      .fn<() => HTMLImageElement>()
      .mockReturnValueOnce(first)
      .mockReturnValueOnce(second);
    const invalidate = jest.fn();
    const logger = { debug: jest.fn(), warn: jest.fn() };
    const cache = new CubeCanvasPreviewImageCache({
      createImage,
      invalidate,
      logger,
    });

    expect(cache.get('/view?filename=failed.png')).toBeNull();
    first.onerror?.(new Event('error'));

    expect(logger.warn).toHaveBeenCalledWith('SugarCubes could not load a Cube preview image.', {
      url: '/view?filename=failed.png',
    });
    expect(invalidate).toHaveBeenCalledTimes(1);
    expect(cache.get('/view?filename=failed.png')).toBeNull();
    expect(createImage).toHaveBeenCalledTimes(2);
  });
});
