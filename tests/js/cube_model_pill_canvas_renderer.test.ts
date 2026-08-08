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
/** Verify Nodes 1.0 model-pill drawing geometry and theme colors. */

import { jest } from '@jest/globals';

import { resolveCubeModelTitle } from '../../frontend/comfyui/ui/cube/CubeModelTitlePresentation.js';
import { CubeModelPillCanvasRenderer } from '../../frontend/comfyui/ui/surface/CubeModelPillCanvasRenderer.js';

test('draws a centered pill shorter than its adjacent title text', () => {
  const context = drawingContext();
  const geometry = new CubeModelPillCanvasRenderer().draw(
    context,
    resolveCubeModelTitle({
      targetModel: 'Anima',
      title: 'Anima/Prompt by Region',
      suffix: 'version 3.2.0',
    }),
    {
      x: 200,
      y: 20,
      maxWidth: 240,
      font: '12px sans-serif',
      fontFamily: 'sans-serif',
      fontSize: 12,
      textColor: '#eee',
      punchoutColor: '#222',
      align: 'center',
    },
  );

  expect(geometry.pillHeight).toBeLessThan(12);
  expect(geometry.startX).toBeLessThan(200);
  expect(context.roundRect).toHaveBeenCalledWith(
    expect.any(Number),
    expect.any(Number),
    expect.any(Number),
    geometry.pillHeight,
    geometry.pillHeight / 2,
  );
  expect(context.fillText).toHaveBeenCalledWith(
    'Anima',
    expect.any(Number),
    20,
    expect.any(Number),
  );
  expect(context.fillText).toHaveBeenCalledWith(
    'Prompt by Region version 3.2.0',
    expect.any(Number),
    20,
    expect.any(Number),
  );
});

test('draws a plain nonmatching title without pill geometry', () => {
  const context = drawingContext();
  const geometry = new CubeModelPillCanvasRenderer().draw(
    context,
    resolveCubeModelTitle({ targetModel: 'Anima', title: 'Custom alias' }),
    {
      x: 10,
      y: 20,
      maxWidth: 200,
      font: '16px sans-serif',
      fontFamily: 'sans-serif',
      fontSize: 16,
      textColor: '#eee',
      punchoutColor: '#222',
      align: 'left',
    },
  );

  expect(geometry.pillHeight).toBe(0);
  expect(context.roundRect).not.toHaveBeenCalled();
  expect(context.fillText).toHaveBeenCalledWith('Custom alias', 10, 20, 200);
});

/** Provide the finite Canvas surface required by the model-title renderer. */
function drawingContext(): CanvasRenderingContext2D {
  return {
    save: jest.fn(),
    restore: jest.fn(),
    beginPath: jest.fn(),
    roundRect: jest.fn(),
    fill: jest.fn(),
    fillText: jest.fn(),
    measureText: jest.fn((text: string) => ({ width: text.length * 6 }) as TextMetrics),
  } as unknown as CanvasRenderingContext2D;
}
