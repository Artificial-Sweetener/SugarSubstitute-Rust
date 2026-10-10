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
/** Prove bounded decorative icon adaptation retains source artwork through browser canvas failures. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { prepareCubeIconContrast } from '../../../frontend/comfyui/ui/core/CubeIconAssetContrast.js';
import {
  createResolvedCubeIconElement,
  FALLBACK_ICON_STYLE,
} from '../../../frontend/comfyui/ui/core/CubeIconResolver.js';

afterEach(() => {
  jest.restoreAllMocks();
});

test('creates one bounded contrast variant while preserving source, alpha and image identity', () => {
  const f = fixture();
  const drawImage = jest.fn();
  const putImageData = jest.fn<(pixels: ImageData) => void>();
  const context = {
    drawImage,
    getImageData: (_x: number, _y: number, width: number, height: number): ImageData => ({
      data: new Uint8ClampedArray(width * height * 4).fill(255),
      width,
      height,
      colorSpace: 'srgb',
    }),
    putImageData,
  };
  // JSDOM provides no canvas renderer; replace only the external methods this adapter uses.
  jest
    .spyOn(HTMLCanvasElement.prototype, 'getContext')
    .mockReturnValue(context as unknown as CanvasRenderingContext2D);
  prepareCubeIconContrast(f.root);
  prepareCubeIconContrast(f.root);
  expect(f.root.dataset.iconTone).toBe('bright');
  const variant = f.root.querySelector('canvas')!;
  expect([variant.width, variant.height]).toEqual([192, 192]);
  expect(variant.getAttribute('aria-hidden')).toBe('true');
  expect(f.root.querySelector('img')).toBe(f.image);
  expect(f.image.getAttribute('src')).toBe('/resolved-icon.png');
  expect(putImageData.mock.calls[0]![0].data.slice(0, 4)).toEqual(
    new Uint8ClampedArray([0, 0, 0, 255]),
  );
  expect(drawImage).toHaveBeenCalledTimes(2);
});

test.each(['unavailable', 'tainted'] as const)('retains source when canvas is %s', (failure) => {
  const f = fixture();
  const warn = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
  const context = jest.spyOn(HTMLCanvasElement.prototype, 'getContext');
  if (failure === 'unavailable') context.mockReturnValue(null);
  else
    context.mockImplementation(() => {
      throw new DOMException('Tainted canvas', 'SecurityError');
    });
  prepareCubeIconContrast(f.root);
  expect(f.root.querySelector('img')).toBe(f.image);
  expect(f.root.querySelector('canvas')).toBeNull();
  expect(f.root.dataset.iconTone).toBe(failure === 'tainted' ? 'original' : undefined);
  expect(warn).toHaveBeenCalledTimes(failure === 'tainted' ? 1 : 0);
});

test('ignores a loading image replaced before load completes', () => {
  const f = fixture(false);
  const context = jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
  prepareCubeIconContrast(f.root);
  f.root.replaceChildren();
  f.image.dispatchEvent(new Event('load'));
  expect(context).not.toHaveBeenCalled();
});

test('prepares decoded icons through the shared element factory for every DOM consumer', () => {
  const context = {
    drawImage: jest.fn(),
    getImageData: (_x: number, _y: number, width: number, height: number): ImageData => ({
      data: new Uint8ClampedArray(width * height * 4).fill(255),
      width,
      height,
      colorSpace: 'srgb',
    }),
    putImageData: jest.fn(),
  };
  // JSDOM has no raster backend; replace only the canvas adapter boundary.
  jest
    .spyOn(HTMLCanvasElement.prototype, 'getContext')
    .mockReturnValue(context as unknown as CanvasRenderingContext2D);
  const root = createResolvedCubeIconElement(document, {
    kind: 'asset',
    url: '/resolved.png',
    mediaType: 'image/png',
    initials: 'TI',
    fallback: FALLBACK_ICON_STYLE,
  });
  const image = root.querySelector('img');
  if (!image) throw new Error('Missing resolved image');
  Object.defineProperties(image, { naturalWidth: { value: 64 }, naturalHeight: { value: 32 } });
  image.dispatchEvent(new Event('load'));
  expect(root.dataset.iconTone).toBe('bright');
  expect(root.querySelector('.sugarcubes-cube-icon__contrast')).not.toBeNull();
  expect(root.querySelector('img')).toBe(image);
});

/** Provide an already resolved image without network access or document observation. */
function fixture(complete = true) {
  const root = document.createElement('span');
  const image = document.createElement('img');
  image.src = '/resolved-icon.png';
  Object.defineProperties(image, {
    complete: { value: complete },
    naturalWidth: { value: 64 },
    naturalHeight: { value: 32 },
  });
  root.append(image);
  return { root, image };
}
