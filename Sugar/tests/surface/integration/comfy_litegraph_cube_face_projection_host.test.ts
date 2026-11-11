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
  type LiteGraphCubeFaceProjectionItem,
} from '../../../frontend/comfyui/ui/surface/ComfyLiteGraphCubeFaceProjectionHost.js';
import type { CubeCanvasRect } from '../../../frontend/comfyui/ui/surface/CubeCanvasLayout.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

/** Supply a typed visible face card without unrelated Cube frame and port geometry. */
function projectionItem(
  node: ComfyNode,
  rect: CubeCanvasRect,
  bodyHeight: number,
): LiteGraphCubeFaceProjectionItem {
  return {
    layout: {
      cards: [
        {
          node,
          id: 'inside',
          label: 'Inside',
          enabled: true,
          showActivationControl: false,
          activationAction: null,
          bodyHeight,
          rect,
        },
      ],
    },
  };
}

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
    const graphHost = document.createElement('div');
    graphHost.append(canvas);
    document.body.append(graphHost);
    const node = { id: 'inside' };
    const host = new ComfyLiteGraphCubeFaceProjectionHost({
      document,
      canvas: { canvas, ds: { scale: 2, offset: [5, 7] } },
    });

    host.sync([projectionItem(node, { x: 100, y: 200, width: 300, height: 180 }, 150)]);

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
    expect(root?.parentElement).toBe(graphHost);
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
    host.sync([projectionItem(node, { x: 0, y: 0, width: 100, height: 120 }, 90)]);

    host.dispose();

    expect(Reflect.get(node, CUBE_FACE_PROJECTION_SYMBOL)).toBe(previous);
    expect(Object.prototype.propertyIsEnumerable.call(node, CUBE_FACE_PROJECTION_SYMBOL)).toBe(
      false,
    );
  });

  test('follows canvas host replacement and detachment without replacing projected controls', () => {
    const canvas = document.createElement('canvas');
    const original = document.createElement('div');
    const replacement = document.createElement('div');
    original.append(canvas);
    document.body.append(original, replacement);
    const node = { id: 'inside' };
    const items = [projectionItem(node, { x: 0, y: 0, width: 100, height: 120 }, 90)];
    const host = new ComfyLiteGraphCubeFaceProjectionHost({ document, canvas: { canvas } });
    try {
      host.sync(items);
      const projection = Reflect.get(node, CUBE_FACE_PROJECTION_SYMBOL) as CubeFaceProjection;
      const root = projection.container.parentElement;
      const control = document.createElement('button');
      projection.container.append(control);
      replacement.append(canvas);
      host.sync(items);
      expect(root?.parentElement).toBe(replacement);
      expect(projection.container.firstElementChild).toBe(control);
      expect(Reflect.get(node, CUBE_FACE_PROJECTION_SYMBOL)).toBe(projection);
      canvas.remove();
      host.sync(items);
      expect(root?.isConnected).toBe(false);
      original.append(canvas);
      host.sync(items);
      expect(root?.parentElement).toBe(original);
      expect(projection.container.firstElementChild).toBe(control);
      expect(document.querySelectorAll('[data-sugarcubes-cube-face-projections]')).toHaveLength(1);
      host.sync([]);
      expect(root?.isConnected).toBe(false);
    } finally {
      host.dispose();
      original.remove();
      replacement.remove();
    }
  });
});
