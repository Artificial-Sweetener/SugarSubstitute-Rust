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
/** Verify native viewport framing stays isolated at the Comfy adapter boundary. */

import { jest } from '@jest/globals';
import { ComfyCanvasGraphFocusAdapter } from '../../frontend/comfyui/ui/surface/ComfyCanvasGraphFocusAdapter.js';

test("frames valid graph bounds through Comfy's transform and requests a repaint", () => {
  const fitToBounds = jest.fn();
  const setDirtyCanvas = jest.fn();
  const adapter = new ComfyCanvasGraphFocusAdapter({ ds: { fitToBounds } }, setDirtyCanvas);

  expect(adapter.focus([80, 120, 860, 260])).toBe(true);
  expect(fitToBounds).toHaveBeenCalledWith([80, 120, 860, 260]);
  expect(setDirtyCanvas).toHaveBeenCalledWith(true, true);
});

test("does not frame invalid bounds or a canvas without Comfy's transform API", () => {
  expect(new ComfyCanvasGraphFocusAdapter({ ds: {} }, jest.fn()).focus([0, 0, 1, 1])).toBe(false);
  expect(
    new ComfyCanvasGraphFocusAdapter({ ds: { fitToBounds() {} } }, jest.fn()).focus([
      0,
      Number.NaN,
      1,
      1,
    ]),
  ).toBe(false);
});

test("prefers Comfy's animated focus path when it is available", () => {
  const animateToBounds = jest.fn();
  const fitToBounds = jest.fn();
  const setDirtyCanvas = jest.fn();
  const adapter = new ComfyCanvasGraphFocusAdapter(
    { ds: { animateToBounds, fitToBounds } },
    setDirtyCanvas,
  );

  expect(adapter.focus([80, 120, 860, 260])).toBe(true);
  expect(animateToBounds).toHaveBeenCalledWith([80, 120, 860, 260], expect.any(Function));
  expect(fitToBounds).not.toHaveBeenCalled();
  const redraw = animateToBounds.mock.calls[0]?.[1];
  if (typeof redraw !== 'function') throw new Error('Expected Comfy redraw callback.');
  redraw();
  expect(setDirtyCanvas).toHaveBeenCalledWith(true, true);
});
