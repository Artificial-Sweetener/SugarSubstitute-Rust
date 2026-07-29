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
import { CubeAuthoringHostCommands } from '../../frontend/comfyui/ui/cube/CubeAuthoringHostCommands.js';

describe('CubeAuthoringHostCommands', () => {
  test("places direct Cube authoring entry points in Comfy's canvas context menu", () => {
    const createCubeFromSelection = jest.fn<() => void>();
    const createCubeFromSubgraph = jest.fn<() => void>();
    const createEmptyCube = jest.fn<() => void>();
    const adapter = new CubeAuthoringHostCommands(
      createCubeFromSelection,
      createCubeFromSubgraph,
      createEmptyCube,
    );

    expect(adapter.getCanvasMenuItems({ selectedItems: { size: 0 } })).toEqual([
      expect.objectContaining({ content: 'Create Empty SugarCube' }),
    ]);
    const items = adapter.getCanvasMenuItems({ selectedItems: { size: 1 } });
    items.forEach((entry) => entry.callback?.());
    expect(createEmptyCube).toHaveBeenCalledTimes(1);
    expect(createCubeFromSelection).toHaveBeenCalledTimes(1);
    expect(createCubeFromSubgraph).toHaveBeenCalledTimes(1);
  });
});
