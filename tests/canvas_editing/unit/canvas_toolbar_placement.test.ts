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
/** Prove the contextual toolbar floats beside its context with stable, clamped placement. */
import { describe, expect, test } from '@jest/globals';
import {
  CanvasToolbarPlacement,
  canvasEditSafeArea,
} from '../../../frontend/comfyui/ui/canvas_editing/domain/CanvasToolbarPlacement.js';

const safe = canvasEditSafeArea({ left: 0, top: 0, width: 800, height: 600 });
const size = { width: 200, height: 36 };

describe('contextual toolbar placement', () => {
  test('insets the safe area by the chrome margin', () => {
    expect(safe).toEqual({ left: 8, top: 8, width: 784, height: 584 });
  });

  test('centres the toolbar twelve pixels below its context', () => {
    const placement = new CanvasToolbarPlacement();
    expect(
      placement.position({ left: 300, top: 100, width: 200, height: 100 }, size, safe, 1),
    ).toEqual({ x: 300, y: 212 });
  });

  test('moves above once the context overflows the bottom and returns only past the margin', () => {
    const placement = new CanvasToolbarPlacement();
    const at = (top: number) =>
      placement.position({ left: 300, top, width: 200, height: 200 }, size, safe, 1).y;
    // Bottom 610 overflows the safe bottom 592 by 18 px: inside the 24 px hysteresis, so below.
    expect(at(410)).toBe(556);
    // Bottom 620 overflows by 28 px: above, twelve pixels over the context.
    expect(at(420)).toBe(372);
    // Still above while within the margin on the way back.
    expect(at(380)).toBe(332);
    // Clear by 24 px: below again.
    expect(at(368)).toBe(556);
    // Physical overflow uses display density: 10 CSS px at density 3 is 30 physical px.
    placement.reset();
    expect(
      placement.position({ left: 300, top: 402, width: 200, height: 200 }, size, safe, 3).y,
    ).toBe(354);
  });

  test('uses the viewport bottom for a context spanning the whole height or no context', () => {
    const placement = new CanvasToolbarPlacement();
    expect(
      placement.position({ left: -50, top: -20, width: 900, height: 700 }, size, safe, 1),
    ).toEqual({ x: 300, y: 556 });
    expect(placement.position(null, size, safe, 1)).toEqual({ x: 300, y: 556 });
  });

  test('clamps the whole toolbar inside the safe area', () => {
    const placement = new CanvasToolbarPlacement();
    expect(
      placement.position({ left: -100, top: 0, width: 20, height: 20 }, size, safe, 1),
    ).toEqual({ x: 8, y: 32 });
    expect(placement.position({ left: 790, top: 0, width: 20, height: 20 }, size, safe, 1)).toEqual(
      { x: 592, y: 32 },
    );
  });
});
