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
/** Verify Nodes 1.0 Cube shells follow LiteGraph's active scene palette. */

import { resolveComfyLiteGraphCubeSurfaceTheme } from '../../frontend/comfyui/ui/surface/ComfyLiteGraphNodeColorTheme.js';

describe('ComfyLiteGraphNodeColorTheme', () => {
  test('uses the active LiteGraph scene colors for an uncolored Cube', () => {
    expect(
      resolveComfyLiteGraphCubeSurfaceTheme(
        {},
        {
          NODE_DEFAULT_COLOR: '#414243',
          NODE_DEFAULT_BGCOLOR: 'rgb(72 73 74)',
        },
      ),
    ).toEqual({
      header: '#414243',
      body: 'rgb(72 73 74)',
    });
  });

  test('lets each explicit Cube color override only its corresponding scene color', () => {
    expect(
      resolveComfyLiteGraphCubeSurfaceTheme(
        { color: '#2b2859' },
        {
          NODE_DEFAULT_COLOR: '#414243',
          NODE_DEFAULT_BGCOLOR: '#353535',
        },
      ),
    ).toEqual({
      header: '#2b2859',
      body: '#353535',
    });
  });

  test('retains LiteGraph native defaults when an older host omits public color constants', () => {
    expect(resolveComfyLiteGraphCubeSurfaceTheme({}, {})).toEqual({
      header: '#333',
      body: '#353535',
    });
  });
});
