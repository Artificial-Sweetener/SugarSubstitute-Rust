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
/** Verify embedded Nodes 1.0 affordances receive exact Cube-face geometry. */

import {
  CUBE_FACE_PROJECTION_SYMBOL,
  ComfyLiteGraphCubeFaceProjectionHost,
  type CubeFaceProjection,
} from '../../frontend/comfyui/ui/surface/ComfyLiteGraphCubeFaceProjectionHost.js';

describe('ComfyLiteGraphCubeFaceProjectionHost', () => {
  test('projects node-local rectangles into a clipped Cube-card surface', () => {
    const canvas = document.createElement('canvas');
    canvas.getBoundingClientRect = () =>
      ({
        x: 10,
        y: 20,
        left: 10,
        top: 20,
        right: 810,
        bottom: 620,
        width: 800,
        height: 600,
        toJSON: () => ({}),
      }) as DOMRect;
    document.body.append(canvas);
    const node = { id: 'inside' };
    const host = new ComfyLiteGraphCubeFaceProjectionHost({
      document,
      canvas: { canvas, ds: { scale: 2, offset: [5, 7] } },
    });

    host.sync([
      {
        layout: {
          cards: [
            {
              node,
              bodyHeight: 150,
              rect: { x: 100, y: 200, width: 300, height: 180 },
            },
          ],
        } as never,
      },
    ]);

    const projection = Reflect.get(node, CUBE_FACE_PROJECTION_SYMBOL) as CubeFaceProjection;
    expect(projection.projectRect([10, 40, 50, 60])).toEqual({
      left: 240,
      top: 574,
      width: 100,
      height: 120,
    });
    expect(projection.container.style.left).toBe('220px');
    expect(projection.container.style.top).toBe('434px');
    expect(projection.container.style.width).toBe('300px');
    expect(projection.container.style.height).toBe('180px');
    expect(projection.container.style.overflow).toBe('hidden');

    const root = document.querySelector<HTMLElement>('[data-sugarcubes-cube-face-projections]');
    canvas.dispatchEvent(new Event('pointerdown'));
    expect(root?.hidden).toBe(true);
    document.dispatchEvent(new Event('pointerup'));
    expect(root?.hidden).toBe(false);

    host.sync([]);
    expect(Reflect.has(node, CUBE_FACE_PROJECTION_SYMBOL)).toBe(false);
    expect(projection.container.isConnected).toBe(false);
    host.dispose();
  });

  test('restores an existing projection descriptor on disposal', () => {
    const canvas = document.createElement('canvas');
    const previous = { owner: 'host' };
    const node = { id: 'inside' };
    Object.defineProperty(node, CUBE_FACE_PROJECTION_SYMBOL, {
      configurable: true,
      enumerable: false,
      value: previous,
    });
    const host = new ComfyLiteGraphCubeFaceProjectionHost({
      document,
      canvas: { canvas },
    });
    host.sync([
      {
        layout: {
          cards: [
            {
              node,
              bodyHeight: 90,
              rect: { x: 0, y: 0, width: 100, height: 120 },
            },
          ],
        } as never,
      },
    ]);

    host.dispose();

    expect(Reflect.get(node, CUBE_FACE_PROJECTION_SYMBOL)).toBe(previous);
    expect(Object.prototype.propertyIsEnumerable.call(node, CUBE_FACE_PROJECTION_SYMBOL)).toBe(
      false,
    );
  });
});
