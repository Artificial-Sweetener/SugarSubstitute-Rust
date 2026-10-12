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
/** Prove large catalogs and exact batch holes produce bounded, correctly ordered gallery windows. */

import { canvasOverviewWindow } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOverviewWindow.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

test('keeps all intersecting cells bounded across narrow, tall, and fractionally scrolled viewports', () => {
  const catalog = canvasCatalogFixture();
  const huge = {
    ...catalog,
    entries: catalog.entries.map((entry) => ({
      ...entry,
      summary: { ...entry.summary, batchCount: 100000 },
    })),
  };
  for (const width of [320, 600, 3840])
    for (const height of [200, 600, 2160, 8000])
      for (const scroll of [0, 0.5, 195, 9999, 10000000]) {
        const window = canvasOverviewWindow(huge, width, height, scroll);
        expect(window.count).toBe(300000);
        expect(window.cells.length).toBeGreaterThan(0);
        expect(window.cells.length).toBeLessThanOrEqual(8);
        expect(window.offset).toBeLessThanOrEqual(scroll);
        expect(new Set(window.cells.map((cell) => cell.key)).size).toBe(window.cells.length);
      }
  const bottom = canvasOverviewWindow(huge, 600, 600, Number.MAX_SAFE_INTEGER);
  expect(bottom.totalHeight).toBe(8_000_000);
  expect(bottom.cells.at(-1)?.selection).toEqual({
    sourceId: huge.entries[2]!.source.id,
    batchIndex: 99999,
  });
});

test('preserves unavailable exact batch positions and source reading order without substituting a final', () => {
  const catalog = canvasCatalogFixture();
  const first = canvasOverviewWindow(catalog, 600, 600, 0);
  expect(first.cells.map((cell) => cell.selection.batchIndex)).toEqual([0, 1, 2, 0, 1, 2]);
  expect(first.cells[0]?.item?.batchIndex).toBe(0);
  expect(first.cells[1]?.item).toBeNull();
  expect(first.cells[3]?.selection.sourceId).toBe(catalog.entries[1]?.source.id);
  const last = canvasOverviewWindow(catalog, 600, 300, Number.MAX_SAFE_INTEGER);
  expect(last.cells.at(-1)?.selection.sourceId).toBe(catalog.entries[2]?.source.id);
  expect(last.cells.at(-1)?.selection.batchIndex).toBe(2);
});

test('refuses invalid viewport geometry and returns no cells for an empty/hidden catalog', () => {
  const catalog = canvasCatalogFixture();
  expect(() => canvasOverviewWindow(catalog, NaN, 600, 0)).toThrow(RangeError);
  expect(() => canvasOverviewWindow(catalog, 600, 600, -1)).toThrow(RangeError);
  expect(canvasOverviewWindow({ ...catalog, entries: [] }, 600, 600, 0).cells).toEqual([]);
  expect(canvasOverviewWindow(catalog, 600, 0, 0).cells).toEqual([]);
});
