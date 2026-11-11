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
/** Verify Nodes 1 delegates native file drops to the internal card under the cursor. */

import { jest } from '@jest/globals';
import { ComfyLiteGraphCubeDropTargetBridge } from '../../../frontend/comfyui/ui/surface/ComfyLiteGraphCubeDropTargetBridge.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

describe('ComfyLiteGraphCubeDropTargetBridge', () => {
  test('delegates through the internal node callbacks and restores the Cube callbacks', async () => {
    const innerDragOver = jest.fn(() => true);
    const innerDragDrop = jest.fn(() => true);
    const inner: ComfyNode = {
      id: 'inner',
      onDragOver: innerDragOver,
      onDragDrop: innerDragDrop,
    };
    const outerDragOver = jest.fn(() => false);
    const outerDragDrop = jest.fn(() => false);
    const cube: ComfyNode = {
      id: 'cube',
      onDragOver: outerDragOver,
      onDragDrop: outerDragDrop,
    };
    const bridge = new ComfyLiteGraphCubeDropTargetBridge();
    bridge.sync([
      {
        node: cube,
        cards: [{ node: inner, rect: { x: 100, y: 120, width: 240, height: 180 } }],
      },
    ]);
    const over = canvasDragEvent('dragover', 180, 200);
    const drop = canvasDragEvent('drop', 180, 200);

    expect(cube.onDragOver?.(over)).toBe(true);
    expect(await cube.onDragDrop?.(drop)).toBe(true);
    expect(innerDragOver).toHaveBeenCalledWith(over);
    expect(innerDragDrop).toHaveBeenCalledWith(drop);
    expect(outerDragOver).not.toHaveBeenCalled();
    expect(outerDragDrop).not.toHaveBeenCalled();

    bridge.sync([]);
    expect(cube.onDragOver).toBe(outerDragOver);
    expect(cube.onDragDrop).toBe(outerDragDrop);
  });

  test('preserves the Cube callback outside internal card bounds', async () => {
    const outerDragOver = jest.fn(() => true);
    const outerDragDrop = jest.fn(() => true);
    const cube: ComfyNode = {
      id: 'cube',
      onDragOver: outerDragOver,
      onDragDrop: outerDragDrop,
    };
    const bridge = new ComfyLiteGraphCubeDropTargetBridge();
    bridge.sync([{ node: cube, cards: [] }]);
    const over = canvasDragEvent('dragover', 20, 30);
    const drop = canvasDragEvent('drop', 20, 30);

    expect(cube.onDragOver?.(over)).toBe(true);
    expect(await cube.onDragDrop?.(drop)).toBe(true);
    expect(outerDragOver).toHaveBeenCalledWith(over);
    expect(outerDragDrop).toHaveBeenCalledWith(drop);
    bridge.dispose();
  });
});

/** Build one Comfy-adjusted drag event with graph-space coordinates. */
function canvasDragEvent(type: string, canvasX: number, canvasY: number): DragEvent {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    canvasX: { value: canvasX },
    canvasY: { value: canvasY },
  });
  return event as DragEvent;
}
