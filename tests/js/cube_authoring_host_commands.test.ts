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
  test('publishes one Comfy command for the visible authoring action', () => {
    const createCube = jest.fn<() => void>();
    const adapter = new CubeAuthoringHostCommands(createCube);

    expect(adapter.commands).toEqual([
      expect.objectContaining({
        id: 'SugarCubes.Graph.ConvertSelectionToCube',
        label: 'Convert Selection to SugarCube',
      }),
    ]);
    adapter.commands[0]?.function();
    expect(createCube).toHaveBeenCalledTimes(1);
  });
});
