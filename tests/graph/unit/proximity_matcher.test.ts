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

import { ProximityMatcher } from '../../../frontend/comfyui/ui/overlays/proximity/ProximityMatcher.js';
import type {
  ProximityEndpointSet,
  ProximityInputEndpoint,
  ProximityOutputEndpoint,
} from '../../../frontend/comfyui/ui/overlays/proximity/ProximityModel.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

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

  test('prefers canonical compatible slots on horizontally adjacent Cubes', () => {
    const outputNode: ComfyNode = {
      id: 10,
      pos: [100, 100],
      size: [400, 320],
      outputs: [],
    };
    const inputNode: ComfyNode = {
      id: 20,
      pos: [540, 100],
      size: [400, 320],
      inputs: [],
    };
    const outputs = [
      outputEndpoint(outputNode, 0, 'MASK', [500, 160]),
      outputEndpoint(outputNode, 1, 'IMAGE', [500, 360]),
    ];
    const inputs = [
      inputEndpoint(inputNode, 0, 'IMAGE', [540, 160]),
      inputEndpoint(inputNode, 1, 'IMAGE', [540, 360]),
    ];
    const matcher = new ProximityMatcher(
      { discover: () => ({ outputs, inputs }) },
      () => ({ isValidConnection: (output, input) => output === input }),
      console,
    );

    expect(
      matcher
        .compute({ _nodes: [outputNode, inputNode] }, { radius: 160, strict: true })
        .map((match) => [match.outputSlot, match.inputSlot]),
    ).toEqual([[1, 0]]);
  });

  test('retains a selected pair inside the release radius and drops it after hysteresis', () => {
    const outputNode: ComfyNode = { id: 10, pos: [100, 100], size: [400, 320] };
    const inputNode: ComfyNode = { id: 20, pos: [540, 100], size: [400, 320] };
    const output = outputEndpoint(outputNode, 0, 'IMAGE', [500, 160]);
    const input = inputEndpoint(inputNode, 0, 'IMAGE', [540, 160]);
    const matcher = new ProximityMatcher(
      { discover: () => ({ outputs: [output], inputs: [input] }) },
      () => ({ isValidConnection: () => true }),
      console,
    );

    expect(matcher.compute({ _nodes: [] }, { radius: 160, strict: true })).toHaveLength(1);
    inputNode.pos = [670, 100];
    input.slotPos = [670, 160];
    expect(matcher.compute({ _nodes: [] }, { radius: 160, strict: true })).toHaveLength(1);
    inputNode.pos = [710, 100];
    input.slotPos = [710, 160];
    expect(matcher.compute({ _nodes: [] }, { radius: 160, strict: true })).toHaveLength(0);
  });

  test('connects distinct nodes even when imported metadata repeats an instance id', () => {
    const outputNode: ComfyNode = { id: 10, pos: [100, 100], size: [400, 320] };
    const inputNode: ComfyNode = { id: 20, pos: [540, 100], size: [400, 320] };
    const output = outputEndpoint(outputNode, 0, 'IMAGE', [500, 160]);
    const input = inputEndpoint(inputNode, 0, 'IMAGE', [540, 160]);
    output.instanceId = 'duplicated-import-id';
    input.instanceId = 'duplicated-import-id';
    const matcher = new ProximityMatcher(
      { discover: () => ({ outputs: [output], inputs: [input] }) },
      () => ({ isValidConnection: () => true }),
      console,
    );

    expect(matcher.compute({ _nodes: [] }, { radius: 160, strict: true })).toHaveLength(1);
  });

  test('checks compatibility only for horizontally nearby inputs', () => {
    const outputNode: ComfyNode = { id: 10, pos: [0, 0], size: [100, 100] };
    const output = outputEndpoint(outputNode, 0, 'IMAGE', [100, 40]);
    const nearbyNode: ComfyNode = { id: 20, pos: [140, 0], size: [100, 100] };
    const nearby = inputEndpoint(nearbyNode, 0, 'IMAGE', [140, 40]);
    const distant = Array.from({ length: 1_000 }, (_, index) => {
      const node: ComfyNode = {
        id: index + 100,
        pos: [10_000 + index * 200, 0],
        size: [100, 100],
      };
      return inputEndpoint(node, 0, 'IMAGE', [10_000 + index * 200, 40]);
    });
    let compatibilityChecks = 0;
    const isValidConnection = (): boolean => {
      compatibilityChecks += 1;
      return true;
    };
    const matcher = new ProximityMatcher(
      { discover: () => ({ outputs: [output], inputs: [...distant, nearby] }) },
      () => ({ isValidConnection }),
      console,
    );

    expect(matcher.compute({ _nodes: [] }, { radius: 160, strict: true })).toHaveLength(1);
    expect(compatibilityChecks).toBe(2);
  });
});

/** Build one canonical Cube output endpoint. */
function outputEndpoint(
  node: ComfyNode,
  slot: number,
  type: string,
  slotPos: [number, number],
): ProximityOutputEndpoint {
  return {
    key: `${String(node.id)}:output:${String(slot)}`,
    endpointId: node.id ?? '',
    node,
    slot,
    cube: 'output-definition',
    instanceId: String(node.id),
    alias: type.toLowerCase(),
    type,
    slotPos,
    slotName: type.toLowerCase(),
    originId: `producer-${String(slot)}`,
    originSlot: slot,
  };
}

/** Build one canonical Cube input endpoint. */
function inputEndpoint(
  node: ComfyNode,
  slot: number,
  type: string,
  slotPos: [number, number],
): ProximityInputEndpoint {
  return {
    key: `${String(node.id)}:input:${String(slot)}`,
    endpointId: node.id ?? '',
    node,
    slot,
    cube: 'input-definition',
    instanceId: String(node.id),
    alias: type.toLowerCase(),
    type,
    slotPos,
    slotName: type.toLowerCase(),
    promptTargets: [{ nodeId: `consumer-${String(slot)}`, inputSlot: slot, inputName: 'image' }],
  };
}
