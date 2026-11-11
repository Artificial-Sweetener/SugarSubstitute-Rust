/** @jest-environment node */
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
/** Prove portable source placement and hierarchical composition ranges independently of a mask loader. */
import { canvasLayersAboveTarget } from '../../../integrations/shared/CanvasSourceLayers.js';
import { placeCanvasRaster } from '../../../integrations/shared/CanvasRasterPlacement.js';
import { decodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import type { CanvasSourceLayer } from '../../../integrations/contracts/CanvasProtocol.generated.js';

/** Describe a native top-to-bottom preorder layer without editor dependencies. */
function layer(id: string, parentId: string | null = null, depth = 0): CanvasSourceLayer {
  return { id, name: id, parentId, depth, visible: true };
}

test('retains chosen nested layer ancestors and excludes each preceding subtree once', () => {
  const layers = [
    layer('top'),
    layer('above-group'),
    layer('above-child', 'above-group', 1),
    layer('grandchild', 'above-child', 2),
    layer('target-group'),
    layer('above-sibling', 'target-group', 1),
    layer('chosen', 'target-group', 1),
    layer('below-sibling', 'target-group', 1),
    layer('bottom'),
  ];
  expect(canvasLayersAboveTarget(layers, 'chosen')).toEqual([
    'top',
    'above-group',
    'above-sibling',
  ]);
  expect(canvasLayersAboveTarget(layers, 'target-group')).toEqual(['top', 'above-group']);
  expect(canvasLayersAboveTarget(layers, 'top')).toEqual([]);
  expect(() => canvasLayersAboveTarget(layers, 'missing')).toThrow('unavailable');
  expect(() => canvasLayersAboveTarget([...layers, layer('top')], 'chosen')).toThrow('duplicate');
  expect(() => canvasLayersAboveTarget([layer('a', 'b'), layer('b', 'a')], 'a')).toThrow('cycle');
  expect(() => canvasLayersAboveTarget([layer('a', 'missing')], 'a')).toThrow('incomplete');
});

test('places a trimmed coverage crop inside the full frame rather than expanding it', () => {
  const pixels = placeCanvasRaster(
    { width: 8, height: 6 },
    {
      width: 2,
      height: 2,
      bounds: { left: 4, top: 2, right: 6, bottom: 4 },
      bytes: Uint8Array.from([25, 50, 100, 200]),
    },
    'coverage8',
    0,
  );
  const values = decodeCanvasRaster(pixels);
  expect([pixels.width, pixels.height]).toEqual([8, 6]);
  expect([...values.slice(20, 22)]).toEqual([25, 50]);
  expect([...values.slice(28, 30)]).toEqual([100, 200]);
  expect([...values].filter((value) => value > 0)).toHaveLength(4);
});

test('preserves straight channels, off-canvas clipping and bounded full-frame thumbnails', () => {
  const pixels = placeCanvasRaster(
    { width: 8, height: 4 },
    {
      width: 2,
      height: 1,
      bounds: { left: -2, top: 0, right: 2, bottom: 2 },
      bytes: Uint8Array.from([1, 2, 3, 0, 80, 90, 100, 128]),
    },
    'rgba8',
    4,
  );
  expect([pixels.width, pixels.height]).toEqual([4, 2]);
  const bytes = decodeCanvasRaster(pixels);
  expect([...bytes.slice(0, 4)]).toEqual([80, 90, 100, 128]);
  expect([...bytes.slice(4)]).toEqual(new Array(28).fill(0));
  const empty = placeCanvasRaster(
    { width: 4, height: 8 },
    {
      width: 0,
      height: 0,
      bounds: { left: 0, top: 0, right: 0, bottom: 0 },
      bytes: new Uint8Array(),
    },
    'rgba8',
    4,
  );
  expect([empty.width, empty.height]).toEqual([2, 4]);
  expect([...decodeCanvasRaster(empty)]).toEqual(new Array(32).fill(0));
});

test('rejects invalid native byte counts, bounds and document geometry', () => {
  const source = {
    width: 1,
    height: 1,
    bounds: { left: 0, top: 0, right: 1, bottom: 1 },
    bytes: new Uint8Array(1),
  };
  expect(() => placeCanvasRaster({ width: 0, height: 1 }, source, 'coverage8', 0)).toThrow(
    'geometry',
  );
  expect(() => placeCanvasRaster({ width: 1, height: 1 }, source, 'rgba8', 0)).toThrow(
    'byte count',
  );
  expect(() =>
    placeCanvasRaster(
      { width: 1, height: 1 },
      { ...source, bounds: { ...source.bounds, right: 0 } },
      'coverage8',
      0,
    ),
  ).toThrow('bounds');
  expect(() => placeCanvasRaster({ width: 1, height: 1 }, source, 'coverage8', 1025)).toThrow(
    'thumbnail',
  );
});
