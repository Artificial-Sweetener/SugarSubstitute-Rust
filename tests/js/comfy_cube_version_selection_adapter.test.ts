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
import { ComfyCubeVersionSelectionAdapter } from '../../frontend/comfyui/ui/cube/version/ComfyCubeVersionSelectionAdapter.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';

describe('ComfyCubeVersionSelectionAdapter', () => {
  test('uses Comfy selection after removing stale replacement identities', () => {
    const source = { id: 'instance' } as CubeNode;
    const target = { id: 'instance' } as CubeNode;
    const selectedItems = new Set<unknown>([source, target]);
    const processSelect = jest.fn((node: CubeNode) => void selectedItems.add(node));
    const adapter = new ComfyCubeVersionSelectionAdapter({
      selectedItems,
      processSelect,
    });

    adapter.replace(source, target);

    expect(processSelect).toHaveBeenCalledWith(target);
    expect([...selectedItems]).toEqual([target]);
  });
});
