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
/** Prove reusable masked return geometry without an editor, browser, or image-difference detector. */
import { prepareCanvasImagePatch } from '../../../integrations/shared/CanvasImageRegions.js';
import {
  encodeCanvasRaster,
  decodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';
import type { CanvasImageMapping } from '../../../integrations/contracts/CanvasImageRegion.js';

/** Frame an intrinsic raster at one explicitly retained document rectangle. */
function mapping(width: number, height: number): CanvasImageMapping {
  return {
    imageSize: { width, height },
    imageBounds: { left: 0, top: 0, width, height },
    documentBounds: { left: 0, top: 0, width, height },
  };
}

/** Supply opaque generated pixels that deliberately differ everywhere outside the mask too. */
function image(width: number, height: number) {
  return encodeCanvasRaster(
    width,
    height,
    'rgba8',
    Uint8Array.from(Array.from({ length: width * height }, () => [80, 120, 160, 255]).flat()),
  );
}

test('returns only the mask-defined inpaint, preserving holes, feathering and its original offset', () => {
  const mask = new Uint8Array(6 * 5);
  mask[1 * 6 + 2] = 255;
  mask[1 * 6 + 4] = 128;
  mask[3 * 6 + 3] = 255;
  const result = prepareCanvasImagePatch(image(6, 5), {
    mapping: mapping(6, 5),
    mask: encodeCanvasRaster(6, 5, 'coverage8', mask),
    coverage: 'mask',
  });
  expect(result?.position).toEqual({ left: 2, top: 1 });
  expect(result?.pixels).toMatchObject({ width: 3, height: 3 });
  if (!result) throw new Error('Expected an inpaint patch');
  const values = decodeCanvasRaster(result.pixels);
  expect([...values.slice(0, 4)]).toEqual([80, 120, 160, 255]);
  expect([...values.slice(4, 8)]).toEqual([0, 0, 0, 0]);
  expect(values[11]).toBe(128);
  expect(values[31]).toBe(255);
  expect(values[35]).toBe(0);
});

test('maps a cropped, scaled, padded input back into document coordinates through the same operation', () => {
  const mask = new Uint8Array(8 * 8);
  for (let y = 2; y < 6; y++) for (let x = 2; x < 6; x++) mask[y * 8 + x] = 255;
  const result = prepareCanvasImagePatch(image(8, 8), {
    mapping: {
      imageSize: { width: 8, height: 8 },
      imageBounds: { left: 2, top: 2, width: 4, height: 4 },
      documentBounds: { left: 110, top: 70, width: 2, height: 2 },
    },
    mask: encodeCanvasRaster(8, 8, 'coverage8', mask),
    coverage: 'mask',
  });
  expect(result?.position).toEqual({ left: 110, top: 70 });
  expect(result?.pixels).toMatchObject({ width: 2, height: 2 });
  if (!result) throw new Error('Expected a positioned crop');
  expect([...decodeCanvasRaster(result.pixels)]).toEqual(
    Array.from({ length: 4 }, () => [80, 120, 160, 255]).flat(),
  );
});

test('does not feather finished composite pixels a second time', () => {
  const result = prepareCanvasImagePatch(image(2, 1), {
    mapping: mapping(2, 1),
    mask: encodeCanvasRaster(2, 1, 'coverage8', new Uint8Array([128, 0])),
    coverage: 'support',
  });
  expect(result?.pixels).toMatchObject({ width: 1, height: 1 });
  if (!result) throw new Error('Expected the already blended pixel');
  expect([...decodeCanvasRaster(result.pixels)]).toEqual([80, 120, 160, 255]);
});

test('produces no layer for an empty mask at either resolution and refuses invalid mappings', () => {
  const region = {
    mapping: mapping(2, 1),
    mask: encodeCanvasRaster(2, 1, 'coverage8', new Uint8Array(2)),
    coverage: 'mask' as const,
  };
  expect(prepareCanvasImagePatch(image(2, 1), region)).toBeNull();
  expect(prepareCanvasImagePatch(image(3, 1), region)).toBeNull();
  expect(() =>
    prepareCanvasImagePatch(image(3, 1), {
      ...region,
      mask: encodeCanvasRaster(3, 1, 'coverage8', new Uint8Array(3)),
    }),
  ).toThrow(/geometry/i);
  expect(() =>
    prepareCanvasImagePatch(image(2, 1), {
      ...region,
      mapping: { ...region.mapping, documentBounds: { left: 0, top: 0, width: 0, height: 1 } },
    }),
  ).toThrow(/mapping/i);
});

test.each([
  [4, 4],
  [1, 1],
])(
  'maps a %s by %s output into the captured region while retaining mask holes and feathering',
  (width, height) => {
    const result = prepareCanvasImagePatch(image(width, height), {
      mapping: {
        imageSize: { width: 2, height: 2 },
        imageBounds: { left: 0, top: 0, width: 2, height: 2 },
        documentBounds: { left: 30, top: 40, width: 2, height: 2 },
      },
      mask: encodeCanvasRaster(2, 2, 'coverage8', new Uint8Array([255, 0, 128, 255])),
      coverage: 'mask',
    });
    expect(result?.position).toEqual({ left: 30, top: 40 });
    expect(result?.pixels).toMatchObject({ width: 2, height: 2 });
    expect([...decodeCanvasRaster(result!.pixels)]).toEqual([
      80, 120, 160, 255, 0, 0, 0, 0, 80, 120, 160, 128, 80, 120, 160, 255,
    ]);
  },
);

test('samples a differently sized output with premultiplied alpha without bleeding hidden colors', () => {
  const result = prepareCanvasImagePatch(
    encodeCanvasRaster(2, 1, 'rgba8', new Uint8Array([255, 0, 0, 0, 0, 0, 255, 255])),
    {
      mapping: mapping(3, 1),
      mask: encodeCanvasRaster(3, 1, 'coverage8', new Uint8Array([255, 255, 255])),
      coverage: 'mask',
    },
  );
  expect([...decodeCanvasRaster(result!.pixels)]).toEqual([
    0, 0, 0, 0, 0, 0, 255, 128, 0, 0, 255, 255,
  ]);
});

test('resamples transparent generated pixels without bleeding their hidden color into the patch', () => {
  const result = prepareCanvasImagePatch(
    encodeCanvasRaster(2, 1, 'rgba8', new Uint8Array([255, 0, 0, 0, 0, 0, 255, 255])),
    {
      mapping: {
        imageSize: { width: 2, height: 1 },
        imageBounds: { left: 0, top: 0, width: 2, height: 1 },
        documentBounds: { left: 10, top: 20, width: 3, height: 1 },
      },
      mask: encodeCanvasRaster(2, 1, 'coverage8', new Uint8Array([255, 255])),
      coverage: 'mask',
    },
  );
  expect(result?.position).toEqual({ left: 10, top: 20 });
  expect([...decodeCanvasRaster(result!.pixels)]).toEqual([
    0, 0, 0, 0, 0, 0, 255, 128, 0, 0, 255, 255,
  ]);
});
