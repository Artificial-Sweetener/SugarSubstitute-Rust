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
/** Keep Comfy's graph background translucent only while the desktop frame owns it. */
import { expect, jest, test } from '@jest/globals';
import {
  DesktopGraphBackdrop,
  GRID_LINE_PEAK_ALPHA,
  translucentGridPixels,
} from '../../../frontend/comfyui/ui/surface/frame/DesktopGraphBackdrop.js';

const PALETTE_GRID = 'data:image/png;base64,palette-grid';
const DERIVED_GRID = 'data:image/png;base64,translucent-grid';

function comfyCanvas(): Record<string, unknown> & { setDirty: jest.Mock } {
  return {
    clear_background_color: '#222',
    background_image: PALETTE_GRID,
    _pattern: 'cached',
    setDirty: jest.fn(),
  };
}

async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

test.each([
  ['dark', true, 255],
  ['light', false, 0],
])('%s grid tiles become theme-inked lines with a fully transparent field', (_name, dark, ink) => {
  const field = [34, 34, 34, 255];
  const minor = [25, 25, 25, 255];
  const major = [20, 20, 20, 255];
  const brighter = [46, 46, 46, 255];
  const pixels = new Uint8ClampedArray([...field, ...field, ...minor, ...major, ...brighter]);
  const output = translucentGridPixels(pixels, dark);
  expect([...output.slice(0, 8)]).toEqual([ink, ink, ink, 0, ink, ink, ink, 0]);
  // The strongest departure from the field, whichever way it points, receives the peak alpha.
  expect(output[15]).toBe(Math.round(GRID_LINE_PEAK_ALPHA * 255));
  // Departures keep their proportions: 9, 14 and 12 levels from the field.
  expect(output[11]! / output[15]!).toBeCloseTo(9 / 14, 1);
  expect(output[19]! / output[15]!).toBeCloseTo(12 / 14, 1);
  for (const channel of [8, 9, 10, 12, 13, 14, 16, 17, 18]) expect(output[channel]).toBe(ink);
});

test('grid ink does not depend on the direction the palette draws its lines', () => {
  const darkerLines = new Uint8ClampedArray([34, 34, 34, 255, 34, 34, 34, 255, 20, 20, 20, 255]);
  const lighterLines = new Uint8ClampedArray([
    221, 221, 221, 255, 221, 221, 221, 255, 235, 235, 235, 255,
  ]);
  for (const dark of [true, false])
    expect([...translucentGridPixels(darkerLines, dark)]).toEqual([
      ...translucentGridPixels(lighterLines, dark),
    ]);
});

test('a tile with no lines is entirely transparent and a translucent line keeps its coverage', () => {
  expect([
    ...translucentGridPixels(new Uint8ClampedArray([5, 5, 5, 255, 5, 5, 5, 255]), true),
  ]).toEqual([255, 255, 255, 0, 255, 255, 255, 0]);
  const half = translucentGridPixels(
    new Uint8ClampedArray([34, 34, 34, 255, 34, 34, 34, 255, 20, 20, 20, 128]),
    true,
  );
  expect(half[11]).toBe(Math.round(GRID_LINE_PEAK_ALPHA * (128 / 255) * 255));
});

test('desktop presentation clears the graph and shows a translucent copy of the palette grid', async () => {
  const canvas = comfyCanvas();
  const deriveGrid = jest.fn(async (source: string, _dark: boolean) => {
    expect(source).toBe(PALETTE_GRID);
    return DERIVED_GRID;
  });
  const backdrop = new DesktopGraphBackdrop({ deriveGrid, report: jest.fn() });
  backdrop.attach({ canvas }, true);
  expect(canvas.clear_background_color).toBe('transparent');
  expect(canvas.background_image).toBeNull();
  await settle();
  expect(canvas.background_image).toBe(DERIVED_GRID);
  expect(canvas._pattern).toBeUndefined();
  expect(canvas.setDirty).toHaveBeenLastCalledWith(true, true);
  backdrop.attach({ canvas }, true);
  expect(deriveGrid).toHaveBeenCalledTimes(1);
  backdrop.detach();
});

test('palette changes are retained for Comfy and restored exactly on detach', async () => {
  const canvas = comfyCanvas();
  const backdrop = new DesktopGraphBackdrop({
    deriveGrid: async (source, dark) => `${source}#${dark ? 'dark' : 'light'}`,
    report: jest.fn(),
  });
  backdrop.attach({ canvas }, true);
  canvas.clear_background_color = '#141414';
  canvas.background_image = 'data:image/png;base64,light-grid';
  expect(canvas.clear_background_color).toBe('transparent');
  await settle();
  expect(canvas.background_image).toBe('data:image/png;base64,light-grid#dark');
  backdrop.detach();
  expect(canvas.clear_background_color).toBe('#141414');
  expect(canvas.background_image).toBe('data:image/png;base64,light-grid');
  canvas.clear_background_color = '#000';
  expect(canvas.clear_background_color).toBe('#000');
});

test('undecodable grids are reported and never replaced by the opaque palette tile', async () => {
  const canvas = comfyCanvas();
  const report = jest.fn();
  const failure = new Error('Comfy grid tile did not decode.');
  const backdrop = new DesktopGraphBackdrop({
    deriveGrid: () => Promise.reject(failure),
    report,
  });
  backdrop.attach({ canvas }, true);
  await settle();
  expect(report).toHaveBeenCalledWith(failure);
  expect(canvas.background_image).toBeNull();
  backdrop.detach();
  expect(canvas.background_image).toBe(PALETTE_GRID);
});

test('apps without a graph canvas leave nothing bound', () => {
  const deriveGrid = jest.fn(async (source: string, _dark: boolean) => source);
  const backdrop = new DesktopGraphBackdrop({ deriveGrid, report: jest.fn() });
  expect(() => backdrop.attach({}, true)).not.toThrow();
  expect(() => backdrop.detach()).not.toThrow();
  expect(deriveGrid).not.toHaveBeenCalled();
});

test('a theme change inks the grid again for the new theme and redraws the canvas', async () => {
  const canvas = comfyCanvas();
  const deriveGrid = jest.fn(
    async (source: string, dark: boolean) => `${source}#${dark ? 'dark' : 'light'}`,
  );
  const backdrop = new DesktopGraphBackdrop({ deriveGrid, report: jest.fn() });
  backdrop.attach({ canvas }, true);
  await settle();
  expect(canvas.background_image).toBe(`${PALETTE_GRID}#dark`);
  canvas.setDirty.mockClear();
  backdrop.attach({ canvas }, false);
  expect(deriveGrid).toHaveBeenLastCalledWith(PALETTE_GRID, false);
  expect(canvas.setDirty).toHaveBeenCalledWith(true, true);
  await settle();
  expect(canvas.background_image).toBe(`${PALETTE_GRID}#light`);
  backdrop.attach({ canvas }, true);
  await settle();
  expect(canvas.background_image).toBe(`${PALETTE_GRID}#dark`);
  expect(deriveGrid).toHaveBeenCalledTimes(2);
  backdrop.detach();
});
