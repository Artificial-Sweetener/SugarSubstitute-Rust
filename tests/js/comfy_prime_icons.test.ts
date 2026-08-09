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
/** Verify Cube chrome delegates icon rendering to Comfy's PrimeIcons vocabulary. */

import { jest } from '@jest/globals';
import {
  comfyPrimeIconGlyph,
  createComfyPrimeIconElement,
  drawComfyPrimeIcon,
} from '../../frontend/comfyui/ui/surface/ComfyPrimeIcons.js';
import { resolveCubeFaceTitlebarActions } from '../../frontend/comfyui/ui/surface/CubeFaceChromeActions.js';

describe('ComfyPrimeIcons', () => {
  test('creates host-styled DOM icons without custom SVG geometry', () => {
    const icon = createComfyPrimeIconElement(document, 'window-maximize');

    expect(icon.tagName).toBe('I');
    expect(icon.classList.contains('pi')).toBe(true);
    expect(icon.classList.contains('pi-window-maximize')).toBe(true);
    expect(icon.getAttribute('aria-hidden')).toBe('true');
    expect(icon.querySelector('svg')).toBeNull();
  });

  test('draws canvas icons with the exact PrimeIcons font and glyph', () => {
    const context = {
      save: jest.fn(),
      restore: jest.fn(),
      fillText: jest.fn(),
    } as unknown as CanvasRenderingContext2D;

    drawComfyPrimeIcon(context, 'window-maximize', 24, 18, 16);

    expect(context.font).toBe("16px 'PrimeIcons'");
    expect(context.fillText).toHaveBeenCalledWith(comfyPrimeIconGlyph('window-maximize'), 24, 18);
    expect(context.save).toHaveBeenCalledTimes(1);
    expect(context.restore).toHaveBeenCalledTimes(1);
  });

  test('describes every Cube titlebar action with a PrimeIcons name', () => {
    const actions = resolveCubeFaceTitlebarActions(
      { instance_id: 'cube-1' },
      {
        onSwapLeft() {},
        onSwapRight() {},
      },
    );

    expect(actions.map(({ key, icon }) => [key, icon])).toEqual([
      ['swap-left', 'arrow-left'],
      ['swap-right', 'arrow-right'],
    ]);
  });
});
