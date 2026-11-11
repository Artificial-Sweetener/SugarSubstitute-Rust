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
/** Verify the Nodes 1.0 Cube activation presenter follows Comfy's BooleanWidget vocabulary. */

import { jest } from '@jest/globals';
import {
  CUBE_CANVAS_ACTIVATION_SIZE,
  drawCubeCanvasActivationControl,
} from '../../../frontend/comfyui/ui/surface/CubeCanvasActivationControl.js';

describe('drawCubeCanvasActivationControl', () => {
  test.each([
    [true, 'Enabled', '#89A'],
    [false, 'Disabled', '#333'],
  ] as const)('draws %s as the native boolean dot with a state label', (enabled, label, color) => {
    const context = drawingContext();

    drawCubeCanvasActivationControl(
      context,
      {
        x: 20,
        y: 30,
        width: CUBE_CANVAS_ACTIVATION_SIZE.width,
        height: CUBE_CANVAS_ACTIVATION_SIZE.height,
      },
      enabled,
    );

    expect(context.fillText).toHaveBeenCalledWith(label, expect.any(Number), 39);
    expect(context.arc).toHaveBeenCalledWith(
      expect.any(Number),
      39,
      CUBE_CANVAS_ACTIVATION_SIZE.height * 0.36,
      0,
      Math.PI * 2,
    );
    expect(context.fill).toHaveBeenCalledTimes(1);
    expect(context.fillStyle).toBe(color);
  });
});

/** Provide the focused Canvas surface used by the activation presenter. */
function drawingContext(): CanvasRenderingContext2D {
  return {
    save: jest.fn(),
    restore: jest.fn(),
    beginPath: jest.fn(),
    arc: jest.fn(),
    fill: jest.fn(),
    fillText: jest.fn(),
  } as unknown as CanvasRenderingContext2D;
}
