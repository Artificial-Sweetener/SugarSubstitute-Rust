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
/** Verify measured Cube constraints change only undersized native geometry. */

import { jest } from '@jest/globals';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { enforceCubeNodeMinimumSize } from '../../../frontend/comfyui/ui/surface/CubeNodeMinimumSizeAdapter.js';

describe('enforceCubeNodeMinimumSize', () => {
  test('expands both undersized dimensions while preserving graph origin', () => {
    const node = cubeNode([80, 120], [320, 200]);

    expect(enforceCubeNodeMinimumSize(node, [440, 475.2])).toBe(true);

    expect([...node.pos]).toEqual([80, 120]);
    expect([...node.size]).toEqual([440, 476]);
    expect(node.setSize).toHaveBeenCalledWith([440, 476]);
    expect(node.onResize).toHaveBeenCalledWith([440, 476]);
  });

  test('does not shrink a node that already fits its contents', () => {
    const node = cubeNode([80, 120], [640, 520]);

    expect(enforceCubeNodeMinimumSize(node, [440, 476])).toBe(false);

    expect([...node.size]).toEqual([640, 520]);
    expect(node.setSize).not.toHaveBeenCalled();
    expect(node.onResize).not.toHaveBeenCalled();
  });
});

/** Build the native geometry boundary needed by the adapter. */
function cubeNode(position: [number, number], size: [number, number]): CubeNode {
  return {
    pos: [...position],
    size: [...size],
    setSize: jest.fn(),
    onResize: jest.fn(),
  } as unknown as CubeNode;
}
