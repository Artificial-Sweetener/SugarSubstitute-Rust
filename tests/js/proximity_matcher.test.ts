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
/** Verify proximity policy independently from Comfy endpoint discovery. */

import { ProximityMatcher } from '../../frontend/comfyui/ui/overlays/proximity/ProximityMatcher.js';
import type {
  ProximityEndpointSet,
  ProximityInputEndpoint,
  ProximityOutputEndpoint,
} from '../../frontend/comfyui/ui/overlays/proximity/ProximityModel.js';
import type { ComfyNode } from '../../frontend/comfyui/ui/types/graph.js';

describe('ProximityMatcher', () => {
  test('matches exact boundary slots and preserves flattened prompt routing', () => {
    const outputNode: ComfyNode = { id: 10, type: 'CubeA' };
    const inputNode: ComfyNode = { id: 20, type: 'CubeB' };
    const output: ProximityOutputEndpoint = {
      key: '10:output:2',
      endpointId: 10,
      node: outputNode,
      slot: 2,
      cube: 'definition-a',
      instanceId: '10',
      alias: 'image',
      type: 'IMAGE',
      slotPos: [100, 40],
      slotName: 'image',
      originId: '10:producer',
      originSlot: 1,
    };
    const input: ProximityInputEndpoint = {
      key: '20:input:1',
      endpointId: 20,
      node: inputNode,
      slot: 1,
      cube: 'definition-b',
      instanceId: '20',
      alias: 'image',
      type: 'IMAGE',
      slotPos: [120, 40],
      slotName: 'image',
      promptTargets: [{ nodeId: '20:nested:consumer', inputSlot: 3, inputName: 'image' }],
    };
    const endpoints: ProximityEndpointSet = { outputs: [output], inputs: [input] };
    const matcher = new ProximityMatcher(
      { discover: () => endpoints },
      () => ({ isValidConnection: () => true }),
      console,
    );

    expect(matcher.compute({ _nodes: [] }, { radius: 160, strict: true })).toEqual([
      expect.objectContaining({
        outputId: 10,
        outputSlot: 2,
        inputId: 20,
        inputSlot: 1,
        originId: '10:producer',
        originSlot: 1,
        promptTargets: [{ nodeId: '20:nested:consumer', inputSlot: 3, inputName: 'image' }],
      }),
    ]);
  });
});
