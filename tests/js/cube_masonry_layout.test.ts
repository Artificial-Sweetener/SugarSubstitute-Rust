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

  test('uses the compact column when column occupancy is already even', () => {
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

  test('matches SugarSubstitute shortest-column compaction even when later cards rise visually', () => {
    const layout = computeCubeMasonry(
      [
        { id: 'positive', height: 100, columnSpan: 2 },
        { id: 'negative', height: 100, columnSpan: 2 },
        { id: 'checkpoint', height: 180 },
        { id: 'encode-style', height: 90 },
        { id: 'mahiro', height: 70 },
        { id: 'sampler', height: 300 },
      ],
      {
        availableWidth: 410,
        minimumColumnWidth: 200,
        gap: 10,
      },
    );
    const byId = new Map(layout.placements.map((placement) => [placement.id, placement]));

    expect(byId.get('checkpoint')?.column).toBe(0);
    expect(byId.get('encode-style')?.column).toBe(1);
    expect(byId.get('mahiro')?.column).toBe(1);
    expect(byId.get('sampler')?.column).toBe(1);
    expect(byId.get('sampler')?.y).toBe(400);
  });

  test('reserves the earliest lowest contiguous window exactly like SugarSubstitute', () => {
    const layout = computeCubeMasonry(
      [
        { id: 'settings', height: 100 },
        { id: 'prompt', height: 80, columnSpan: 2 },
        { id: 'sampler', height: 60 },
      ],
      {
        availableWidth: 410,
        minimumColumnWidth: 200,
        gap: 10,
      },
    );

    expect(layout.placements).toEqual([
      { id: 'settings', column: 0, x: 0, y: 0, width: 200, height: 100 },
      { id: 'prompt', column: 0, x: 0, y: 110, width: 410, height: 80 },
      { id: 'sampler', column: 0, x: 0, y: 200, width: 200, height: 60 },
    ]);
    expect(layout.height).toBe(260);
  });

  test('derives columns from width and uses SugarSubstitute integer column widths', () => {
    const layout = computeCubeMasonry(
      [
        { id: 'a', height: 100 },
        { id: 'b', height: 100 },
      ],
      {
        availableWidth: 652,
        minimumColumnWidth: 200,
        gap: 10,
      },
    );

    expect(layout.columnCount).toBe(3);
    expect(layout.columnWidth).toBe(210);
    expect(layout.placements).toEqual([
      { id: 'a', column: 0, x: 0, y: 0, width: 210, height: 100 },
      { id: 'b', column: 1, x: 220, y: 0, width: 210, height: 100 },
    ]);
  });

  test('falls back to one column when a prompt card cannot span two columns', () => {
    const layout = computeCubeMasonry([{ id: 'prompt', height: 80, columnSpan: 2 }], {
      availableWidth: 200,
      minimumColumnWidth: 200,
      gap: 10,
    });

    expect(layout.placements[0]).toEqual({
      id: 'prompt',
      column: 0,
      x: 0,
      y: 0,
      width: 200,
      height: 80,
    });
  });

  test('distributes cards from the authoritative insertion order', () => {
    const layout = computeCubeMasonry(
      [
        { id: 'first', height: 180 },
        { id: 'second', height: 300 },
        { id: 'third', height: 70 },
        { id: 'fourth', height: 90 },
      ],
      {
        availableWidth: 650,
        minimumColumnWidth: 200,
        gap: 10,
      },
    );

    expect(layout.placements.map(({ id, column, y }) => ({ id, column, y }))).toEqual([
      { id: 'first', column: 0, y: 0 },
      { id: 'second', column: 1, y: 0 },
      { id: 'third', column: 2, y: 0 },
      { id: 'fourth', column: 2, y: 80 },
    ]);
  });
});

describe('orderCubeSurfaceCards', () => {
  test('honors persisted order and appends newly discovered nodes', () => {
    expect(orderCubeSurfaceCards(['c', 'missing', 'a'], ['a', 'b', 'c'])).toEqual(['c', 'a', 'b']);
  });
});
