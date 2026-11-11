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
/** Prove definition disposal releases both Comfy registries through one host adapter. */
import { expect, jest, test } from '@jest/globals';
import { discardNativeCubeDefinition } from '../../../frontend/comfyui/ui/cube/graph/NativeCubeDefinitionRegistry.js';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';

test('unregisters an allocated type even when partial creation did not retain a definition entry', () => {
  const subgraphs = new Map<string, NativeCubeSubgraph>();
  const unregisterNodeType = jest.fn();
  discardNativeCubeDefinition({ subgraphs }, { unregisterNodeType }, 'allocated');
  expect(subgraphs.has('allocated')).toBe(false);
  expect(unregisterNodeType).toHaveBeenCalledWith('allocated');
});

test('preserves native cleanup failures for the enclosing mutation to report', () => {
  const unregisterNodeType = jest.fn(() => {
    throw new Error('native unregister failed');
  });
  expect(() =>
    discardNativeCubeDefinition({ subgraphs: new Map() }, { unregisterNodeType }, 'allocated'),
  ).toThrow('native unregister failed');
});
