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
/** Prove a dragged item's slot is decided from settled geometry, for both kinds of list. */
import { expect, test } from '@jest/globals';
import {
  extentPlacement,
  slotOffsets,
  strideProjection,
} from '../../../frontend/comfyui/ui/interaction/reorder/ReorderSlots.js';

test('items laid end to end start where the ones before them end', () => {
  expect(slotOffsets([80, 200, 120])).toEqual([0, 80, 280, 400]);
  expect(slotOffsets([])).toEqual([0]);
});

test('an item dragged along unequal neighbours displaces them by its own extent', () => {
  expect(extentPlacement([180, 180, 180], 0, 180)).toEqual({
    target: 1,
    draggedStart: 180,
    offsets: [180, -180, 0],
  });
  expect(extentPlacement([80, 200, 120], 0, 200)).toEqual({
    target: 1,
    draggedStart: 200,
    offsets: [200, -80, 0],
  });
  // The dragged edge never leaves the extent the items occupy.
  expect(extentPlacement([180, 180, 180], 0, -400).draggedStart).toBe(0);
  expect(extentPlacement([180, 180, 180], 0, 999).draggedStart).toBe(360);
  expect(() => extentPlacement([], 0, 0)).toThrow('Reorder geometry is unavailable.');
  expect(() => extentPlacement([10, 0], 0, 0)).toThrow('Reorder geometry is unavailable.');
});

test('a pointer resting on a boundary does not flip the order back and forth', () => {
  // Moving forward needs the midpoint plus the dead band; moving back needs it minus the band.
  expect(extentPlacement([180, 180, 180], 0, 97, 0).target).toBe(1);
  expect(extentPlacement([180, 180, 180], 0, 89, 1).target).toBe(1);
  expect(extentPlacement([180, 180, 180], 0, 84, 1).target).toBe(0);
  // A caller may ask for a different band.
  expect(extentPlacement([180, 180, 180], 0, 91, 0, 0).target).toBe(1);
  expect(extentPlacement([180, 180, 180], 0, 91, 0, 20).target).toBe(0);
});

test('a lifted row takes the slot its centre is over, among rows a fixed stride apart', () => {
  const rows = ['a', 'b', 'c', 'd'];
  const project = (key: string, center: number, current = rows, locked: string[] = []) =>
    strideProjection({
      current,
      movable: (item) => !locked.includes(item),
      key,
      center,
      listStart: 100,
      stride: 50,
      size: 46,
    });
  // Row centres sit at 123, 173, 223 and 273.
  expect(project('a', 180)).toEqual(['b', 'a', 'c', 'd']);
  expect(project('a', 400)).toEqual(['b', 'c', 'd', 'a']);
  expect(project('d', 0)).toEqual(['d', 'a', 'b', 'c']);
  // Where it already is, nothing changes and nothing is announced.
  expect(project('b', 173)).toBeNull();
});

test('a projected row has to cross a whole stride to move back', () => {
  const moved = ['b', 'a', 'c'];
  const project = (center: number) =>
    strideProjection({
      current: moved,
      movable: () => true,
      key: 'a',
      center,
      listStart: 0,
      stride: 50,
      size: 46,
    });
  // 'b' now sits in the first slot, centred at 23: only passing that centre moves 'a' back.
  expect(project(30)).toBeNull();
  expect(project(22)).toEqual(['a', 'b', 'c']);
});

test('an item that may not move keeps its slot while the others pass around it', () => {
  expect(
    strideProjection({
      current: ['a', 'locked', 'b', 'c'],
      movable: (item) => item !== 'locked',
      key: 'a',
      center: 1000,
      listStart: 0,
      stride: 50,
      size: 46,
    }),
  ).toEqual(['b', 'locked', 'c', 'a']);
});
