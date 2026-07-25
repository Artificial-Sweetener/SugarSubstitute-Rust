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
/** Verify responsive finite masonry placement for Cube surface cards. */

import { describe, expect, test } from '@jest/globals';

import {
  computeCubeMasonry,
  orderCubeSurfaceCards,
} from '../../frontend/comfyui/ui/surface/CubeMasonryLayout.js';

describe('computeCubeMasonry', () => {
  test('adds columns as the Cube face becomes wider', () => {
    const cards = Array.from({ length: 5 }, (_, index) => ({
      id: `node-${index}`,
      height: 100,
    }));

    expect(
      computeCubeMasonry(cards, {
        availableWidth: 650,
        minimumColumnWidth: 200,
        gap: 10,
      }).columnCount,
    ).toBe(3);
    expect(
      computeCubeMasonry(cards, {
        availableWidth: 1_050,
        minimumColumnWidth: 200,
        gap: 10,
      }).columnCount,
    ).toBe(5);
  });

  test('places each card in the currently shortest column', () => {
    const layout = computeCubeMasonry(
      [
        { id: 'a', height: 300 },
        { id: 'b', height: 100 },
        { id: 'c', height: 100 },
      ],
      {
        availableWidth: 410,
        minimumColumnWidth: 200,
        gap: 10,
      },
    );

    expect(layout.placements).toEqual([
      { id: 'a', column: 0, x: 0, y: 0, width: 200, height: 300 },
      { id: 'b', column: 1, x: 210, y: 0, width: 200, height: 100 },
      { id: 'c', column: 1, x: 210, y: 110, width: 200, height: 100 },
    ]);
    expect(layout.height).toBe(300);
    expect(Number.isFinite(layout.height)).toBe(true);
  });

  test('preserves saved editor columns and vertical order when spatial geometry is available', () => {
    const layout = computeCubeMasonry(
      [
        { id: 'checkpoint', height: 180, sourceX: 0, sourceY: 560, sourceWidth: 320 },
        { id: 'ksampler', height: 300, sourceX: 620, sourceY: 0, sourceWidth: 270 },
        { id: 'mahiro', height: 70, sourceX: 350, sourceY: 0, sourceWidth: 190 },
        { id: 'negative', height: 190, sourceX: 0, sourceY: 340, sourceWidth: 320 },
        { id: 'positive', height: 210, sourceX: 0, sourceY: 110, sourceWidth: 320 },
        { id: 'schedule', height: 150, sourceX: 350, sourceY: 50, sourceWidth: 287 },
        { id: 'style', height: 90, sourceX: 0, sourceY: 0, sourceWidth: 320 },
        { id: 'vae', height: 80, sourceX: 760, sourceY: 310, sourceWidth: 140 },
        { id: 'vectorscope', height: 280, sourceX: 350, sourceY: 100, sourceWidth: 220 },
      ],
      {
        availableWidth: 740,
        minimumColumnWidth: 240,
        gap: 10,
      },
    );
    const byId = new Map(layout.placements.map((placement) => [placement.id, placement]));

    expect(layout.columnCount).toBe(3);
    expect(byId.get('style')?.column).toBe(0);
    expect(byId.get('positive')?.column).toBe(0);
    expect(byId.get('negative')?.column).toBe(0);
    expect(byId.get('checkpoint')?.column).toBe(0);
    expect(byId.get('mahiro')?.column).toBe(1);
    expect(byId.get('schedule')?.column).toBe(1);
    expect(byId.get('vectorscope')?.column).toBe(1);
    expect(byId.get('ksampler')?.column).toBe(2);
    expect(byId.get('vae')?.column).toBe(2);
    expect(byId.get('style')?.y).toBeLessThan(byId.get('positive')?.y ?? 0);
    expect(byId.get('ksampler')?.y).toBeLessThan(byId.get('vae')?.y ?? 0);
  });
});

describe('orderCubeSurfaceCards', () => {
  test('honors persisted order and appends newly discovered nodes', () => {
    expect(orderCubeSurfaceCards(['c', 'missing', 'a'], ['a', 'b', 'c'])).toEqual(['c', 'a', 'b']);
  });
});
