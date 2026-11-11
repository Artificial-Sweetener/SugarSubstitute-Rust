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
/** Prove processing-frame crops retain exact mask pixels and document placement. */
import {
  cropCanvasImageRegion,
  resolveCanvasImageRegion,
} from '../../../integrations/shared/CanvasImageRegionCrop.js';
import { prepareCanvasImagePatch } from '../../../integrations/shared/CanvasImageRegions.js';
import {
  encodeCanvasRaster,
  decodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';

test.each([
  [32, 32, 0, 2],
  [16, 32, 8, 2],
])(
  'uses observed %s by %s VAE dimensions without assuming its compression ratio',
  (width, height, left, top) => {
    const region = {
      outputFrame: 'vae-center-crop' as const,
      mapping: {
        imageSize: { width: 32, height: 37 },
        imageBounds: { left: 0, top: 0, width: 32, height: 37 },
        documentBounds: { left: 100, top: 200, width: 32, height: 37 },
      },
      mask: encodeCanvasRaster(32, 37, 'coverage8', new Uint8Array(32 * 37).fill(127)),
      coverage: 'mask' as const,
    };
    const result = resolveCanvasImageRegion(
      encodeCanvasRaster(width, height, 'rgba8', new Uint8Array(width * height * 4)),
      region,
    );
    expect(result.mapping.documentBounds).toEqual({
      left: 100 + left,
      top: 200 + top,
      width,
      height,
    });
    expect(result.mask).toMatchObject({ width, height });
    expect(new Set(decodeCanvasRaster(result.mask))).toEqual(new Set([127]));
  },
);

test('returns the real 816 by 680 VAE crop without stretching a single pixel', () => {
  const width = 816,
    height = 684;
  const mask = new Uint8Array(width * height);
  mask[2 * width + 5] = 127;
  mask[3 * width + 6] = 255;
  mask[0] = 255;
  mask[683 * width] = 255;
  const region = cropCanvasImageRegion(
    {
      mapping: {
        imageSize: { width, height },
        imageBounds: { left: 0, top: 0, width, height },
        documentBounds: { left: 110, top: 200, width, height },
      },
      mask: encodeCanvasRaster(width, height, 'coverage8', mask),
      coverage: 'mask',
    },
    { left: 0, top: 2, width, height: 680 },
  );
  expect(region.mapping).toEqual({
    imageSize: { width, height: 680 },
    imageBounds: { left: 0, top: 0, width, height: 680 },
    documentBounds: { left: 110, top: 202, width, height: 680 },
  });
  expect([...decodeCanvasRaster(region.mask)].filter(Boolean)).toEqual([127, 255]);
  const image = new Uint8Array(width * 680 * 4);
  image.set([10, 20, 30, 255], 5 * 4);
  image.set([70, 80, 90, 255], (width + 6) * 4);
  const patch = prepareCanvasImagePatch(encodeCanvasRaster(width, 680, 'rgba8', image), region);
  expect(patch?.position).toEqual({ left: 115, top: 202 });
  expect(patch?.pixels).toMatchObject({ width: 2, height: 2 });
  expect([...decodeCanvasRaster(patch!.pixels)]).toEqual([
    10, 20, 30, 127, 0, 0, 0, 0, 0, 0, 0, 0, 70, 80, 90, 255,
  ]);
});

test('intersects padded image bounds and projects an existing document scale', () => {
  const region = cropCanvasImageRegion(
    {
      mapping: {
        imageSize: { width: 13, height: 13 },
        imageBounds: { left: 1, top: 1, width: 10, height: 10 },
        documentBounds: { left: 100, top: 200, width: 20, height: 30 },
      },
      mask: encodeCanvasRaster(13, 13, 'coverage8', new Uint8Array(169).fill(128)),
      coverage: 'support',
    },
    { left: 2, top: 2, width: 8, height: 8 },
  );
  expect(region.mapping).toEqual({
    imageSize: { width: 8, height: 8 },
    imageBounds: { left: 0, top: 0, width: 8, height: 8 },
    documentBounds: { left: 102, top: 203, width: 16, height: 24 },
  });
  expect(region.coverage).toBe('support');
  expect(() => cropCanvasImageRegion(region, { left: 1, top: 0, width: 8, height: 8 })).toThrow(
    /crop/i,
  );
});
