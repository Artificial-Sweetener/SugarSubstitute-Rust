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
/** Prove portable isolated-layer visibility retains ancestry and native child visibility. */
import { canvasLayerIsolation } from '../../../integrations/shared/CanvasSourceLayers.js';
import type { CanvasSourceLayer } from '../../../integrations/contracts/CanvasProtocol.generated.js';

const layers: readonly CanvasSourceLayer[] = [
  { id: 'top', parentId: null, depth: 0, name: 'Top', visible: true },
  { id: 'group', parentId: null, depth: 0, name: 'Group', visible: false },
  { id: 'first', parentId: 'group', depth: 1, name: 'First', visible: true },
  { id: 'nested', parentId: 'group', depth: 1, name: 'Nested', visible: true },
  { id: 'hidden', parentId: 'nested', depth: 2, name: 'Hidden', visible: false },
  { id: 'visible', parentId: 'nested', depth: 2, name: 'Visible', visible: true },
  { id: 'bottom', parentId: null, depth: 0, name: 'Bottom', visible: true },
];

test('isolates a hidden group while preserving child visibility', () => {
  expect(canvasLayerIsolation(layers, 'nested')).toEqual([
    { id: 'top', visible: false },
    { id: 'group', visible: true },
    { id: 'first', visible: false },
    { id: 'nested', visible: true },
    { id: 'hidden', visible: false },
    { id: 'visible', visible: true },
    { id: 'bottom', visible: false },
  ]);
});

test('includes a hidden selected leaf and its ancestors without siblings', () => {
  expect(
    canvasLayerIsolation(layers, 'hidden')
      .filter((item) => item.visible)
      .map((item) => item.id),
  ).toEqual(['group', 'nested', 'hidden']);
  expect(layers[1]?.visible).toBe(false);
});

test('rejects unavailable layers, duplicates and incomplete or cyclic ancestry', () => {
  expect(() => canvasLayerIsolation(layers, 'missing')).toThrow();
  const first = layers[0];
  if (!first) throw new Error('Missing fixture layer.');
  expect(() => canvasLayerIsolation([...layers, first], 'top')).toThrow();
  expect(() => canvasLayerIsolation([{ ...first, parentId: 'missing' }], 'top')).toThrow();
  expect(() => canvasLayerIsolation([{ ...first, parentId: 'top' }], 'top')).toThrow();
});
