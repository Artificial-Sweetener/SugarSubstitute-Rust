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
/** Verify Nodes 1.0 native slots follow Cube preview-title geometry. */

import { jest } from '@jest/globals';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { ComfyLiteGraphCubeBoundaryHost } from '../../../frontend/comfyui/ui/surface/ComfyLiteGraphCubeBoundaryHost.js';

describe('ComfyLiteGraphCubeBoundaryHost', () => {
  test('moves native output dots to title rows with Comfy native horizontal inset', () => {
    const first = { name: 'output.image', type: 'IMAGE', label: 'Image', pos: [12, 34] };
    const second: { name: string; type: string; label?: unknown; pos?: unknown } = {
      name: 'output.mask',
      type: 'MASK',
    };
    const node = {
      pos: [100, 140],
      size: [760, 500],
      outputs: [first, second],
    } as unknown as CubeNode;
    const host = new ComfyLiteGraphCubeBoundaryHost({ slotHeight: 20 });

    host.sync(
      node,
      [],
      [
        port(0, 'output.image', 'IMAGE', 820, 190, first),
        port(1, 'output.mask', 'MASK', 820, 410, second),
      ],
    );

    expect(first.pos).toEqual([751, 50]);
    expect(second.pos).toEqual([751, 270]);
    expect(first.label).toBe('Image');
    expect('label' in second).toBe(false);

    host.release(node);
    expect(first.pos).toEqual([12, 34]);
    expect(first.label).toBe('Image');
    expect('pos' in second).toBe(false);
    expect('label' in second).toBe(false);
  });

  test('clips native slot drawing to moved input and output dots', () => {
    const host = new ComfyLiteGraphCubeBoundaryHost();
    const input: { name: string; type: string; pos?: unknown } = {
      name: 'image',
      type: 'IMAGE',
    };
    const output = { name: 'output.image', type: 'IMAGE', pos: [720, 50] };
    const node = {
      pos: [100, 140],
      size: [760, 500],
      inputs: [input],
      outputs: [output],
    } as unknown as CubeNode;
    const context = {
      save: jest.fn(),
      beginPath: jest.fn(),
      rect: jest.fn(),
      moveTo: jest.fn(),
      arc: jest.fn(),
      clip: jest.fn(),
      restore: jest.fn(),
    } as unknown as CanvasRenderingContext2D;
    const drawSlots = jest.fn();

    host.sync(
      node,
      [port(0, 'image', 'IMAGE', 100, 220, input)],
      [port(0, 'output.image', 'IMAGE', 860, 190, output)],
    );
    host.drawNativeSlotDots(node, context, drawSlots);

    expect(input.pos).toEqual([9, 80]);
    expect(context.arc).toHaveBeenCalledWith(9, 80, 8, 0, Math.PI * 2);
    expect(context.arc).toHaveBeenCalledWith(751, 50, 8, 0, Math.PI * 2);
    expect(context.clip).toHaveBeenCalledTimes(1);
    expect(drawSlots).toHaveBeenCalledTimes(1);
    expect(context.restore).toHaveBeenCalledTimes(1);
  });
});

/** Build complete canvas presentation geometry for one test slot. */
function port(index: number, name: string, type: string, x: number, y: number, slotValue: unknown) {
  return {
    index,
    name,
    type,
    x,
    y,
    defaultY: y,
    minY: 150,
    maxY: 620,
    labelY: y,
    slot: slotValue,
  };
}
