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
/** Prove immutable raster transformation reuse, invalidation, bounded retention and failure recovery. */
import { jest } from '@jest/globals';
import { InputRasterTransformCache } from '../../../frontend/comfyui/ui/input_canvas/application/InputRasterTransformCache.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';

/** Keep equality independent of snapshot object identity. */
function raster(value = 64) {
  return encodeCanvasRaster(2, 2, 'coverage8', new Uint8Array(4).fill(value));
}

test('shares pending and completed pixels across captures and keys every parameter and raster dimension', async () => {
  const cache = new InputRasterTransformCache<object>(),
    value = {},
    load = jest.fn(async () => value);
  const first = cache.read([raster()], 'red:crop1', load, () => 0);
  expect(cache.read([raster()], 'red:crop1', load, () => 0)).toBe(first);
  expect(await first).toBe(value);
  expect(await cache.read([raster()], 'red:crop1', load, () => 0)).toBe(value);
  expect(load).toHaveBeenCalledTimes(1);
  await cache.read([raster(65)], 'red:crop1', load, () => 0);
  await cache.read([raster()], 'alpha:crop1', load, () => 0);
  await cache.read([{ ...raster(), width: 1, height: 4 }], 'red:crop1', load, () => 0);
  expect(load).toHaveBeenCalledTimes(4);
});

test('evicts by bytes and recency while allowing oversized work without retaining it', async () => {
  const cache = new InputRasterTransformCache<number>(40),
    load = jest.fn(async () => 1);
  await cache.read([raster(1)], '', load, () => 0);
  await cache.read([raster(2)], '', load, () => 0);
  await cache.read([raster(1)], '', load, () => 0);
  await cache.read([raster(3)], '', load, () => 0);
  await cache.read([raster(2)], '', load, () => 0);
  expect(load).toHaveBeenCalledTimes(4);
  await cache.read([raster(4)], '', load, () => 41);
  await cache.read([raster(4)], '', load, () => 41);
  expect(load).toHaveBeenCalledTimes(6);
});

test('rejects excess pending work, does not retain failures, and fences results after clearing', async () => {
  const cache = new InputRasterTransformCache<number>(64, 1);
  let finish: (value: number) => void = () => {};
  const first = cache.read(
    [raster()],
    '',
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
    () => 0,
  );
  await expect(
    cache.read(
      [raster(65)],
      '',
      async () => 2,
      () => 0,
    ),
  ).rejects.toThrow('Too many');
  cache.clear();
  finish(1);
  await first;
  const fail = jest.fn(async () => {
    throw new Error('decode failed');
  });
  await expect(cache.read([raster()], '', fail, () => 0)).rejects.toThrow('decode failed');
  await expect(cache.read([raster()], '', fail, () => 0)).rejects.toThrow('decode failed');
  expect(fail).toHaveBeenCalledTimes(2);
});
