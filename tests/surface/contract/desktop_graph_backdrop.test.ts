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

test('grid tiles keep line contrast while their dominant field becomes transparent', () => {
  const field = [34, 34, 34, 255];
  const line = [53, 53, 53, 255];
  const shadow = [17, 17, 17, 255];
  const pixels = new Uint8ClampedArray([...field, ...field, ...field, ...line, ...shadow]);
  const output = translucentGridPixels(pixels);
  expect([...output.slice(0, 4)]).toEqual([255, 255, 255, 0]);
  const lineAlpha = output[15] / 255;
  expect([...output.slice(12, 15)]).toEqual([255, 255, 255]);
  expect(Math.round(34 + lineAlpha * (255 - 34))).toBe(53);
  const shadowAlpha = output[19] / 255;
  expect([...output.slice(16, 19)]).toEqual([0, 0, 0]);
  expect(Math.round(34 * (1 - shadowAlpha))).toBe(17);
});

test('desktop presentation clears the graph and shows a translucent copy of the palette grid', async () => {
  const canvas = comfyCanvas();
  const deriveGrid = jest.fn(async (source: string) => {
    expect(source).toBe(PALETTE_GRID);
    return DERIVED_GRID;
  });
  const backdrop = new DesktopGraphBackdrop({ deriveGrid, report: jest.fn() });
  backdrop.attach({ canvas });
  expect(canvas.clear_background_color).toBe('transparent');
  expect(canvas.background_image).toBeNull();
  await settle();
  expect(canvas.background_image).toBe(DERIVED_GRID);
  expect(canvas._pattern).toBeUndefined();
  expect(canvas.setDirty).toHaveBeenLastCalledWith(true, true);
  backdrop.attach({ canvas });
  expect(deriveGrid).toHaveBeenCalledTimes(1);
  backdrop.detach();
});

test('palette changes are retained for Comfy and restored exactly on detach', async () => {
  const canvas = comfyCanvas();
  const backdrop = new DesktopGraphBackdrop({
    deriveGrid: async (source) => `${source}#translucent`,
    report: jest.fn(),
  });
  backdrop.attach({ canvas });
  canvas.clear_background_color = '#141414';
  canvas.background_image = 'data:image/png;base64,light-grid';
  expect(canvas.clear_background_color).toBe('transparent');
  await settle();
  expect(canvas.background_image).toBe('data:image/png;base64,light-grid#translucent');
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
  backdrop.attach({ canvas });
  await settle();
  expect(report).toHaveBeenCalledWith(failure);
  expect(canvas.background_image).toBeNull();
  backdrop.detach();
  expect(canvas.background_image).toBe(PALETTE_GRID);
});

test('apps without a graph canvas leave nothing bound', () => {
  const deriveGrid = jest.fn(async (source: string) => source);
  const backdrop = new DesktopGraphBackdrop({ deriveGrid, report: jest.fn() });
  expect(() => backdrop.attach({})).not.toThrow();
  expect(() => backdrop.detach()).not.toThrow();
  expect(deriveGrid).not.toHaveBeenCalled();
});
