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
import { describe, expect, jest, test } from '@jest/globals';
import { drawFallbackInitialsCanvas } from '../../frontend/comfyui/ui/core/CubeFallbackIconRenderer.js';

/**
 * Build the minimal canvas context needed to inspect fallback icon drawing.
 */
function createCanvasContext(measurement: TextMetrics) {
  const context = {
    font: '',
    fillStyle: '',
    textAlign: '',
    textBaseline: '',
    save: jest.fn(),
    restore: jest.fn(),
    translate: jest.fn(),
    scale: jest.fn(),
    measureText: jest.fn(() => measurement),
    fillText: jest.fn(),
  };
  return context as typeof context & CanvasRenderingContext2D;
}

describe('cube fallback icon renderer', () => {
  test('normalizes initials size instead of upscaling narrow letter pairs', () => {
    const narrowContext = createCanvasContext({
      width: 50,
      actualBoundingBoxAscent: 56,
      actualBoundingBoxDescent: 4,
      actualBoundingBoxLeft: 0,
      actualBoundingBoxRight: 50,
    } as TextMetrics);
    const wideContext = createCanvasContext({
      width: 90,
      actualBoundingBoxAscent: 56,
      actualBoundingBoxDescent: 4,
      actualBoundingBoxLeft: 0,
      actualBoundingBoxRight: 90,
    } as TextMetrics);

    drawFallbackInitialsCanvas(narrowContext, { initials: 'TI' }, 0, 0, 96);
    drawFallbackInitialsCanvas(wideContext, { initials: 'DA' }, 0, 0, 96);

    expect(narrowContext.font).toContain('62px');
    expect(wideContext.font).toContain('62px');
    expect(narrowContext.scale.mock.calls).toContainEqual([1, 1]);
    expect(wideContext.scale.mock.calls).toContainEqual([1, 1]);
  });

  test('shrinks only when initials exceed the fallback icon footprint', () => {
    const context = createCanvasContext({
      width: 130,
      actualBoundingBoxAscent: 56,
      actualBoundingBoxDescent: 4,
      actualBoundingBoxLeft: 0,
      actualBoundingBoxRight: 130,
    } as TextMetrics);

    drawFallbackInitialsCanvas(context, { initials: 'WW' }, 0, 0, 96);

    expect(context.font).toContain('62px');
    expect(context.scale.mock.calls).toContainEqual([1, 1]);
    expect(context.scale.mock.calls).toContainEqual([92 / 130, 92 / 130]);
  });
});
