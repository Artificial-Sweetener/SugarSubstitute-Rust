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
/** Verify native Cube proximity endpoints follow Comfy's real subgraph boundaries. */

import { jest } from '@jest/globals';
import { NativeCubeProximityEndpointSource } from '../../frontend/comfyui/ui/cube/connection/NativeCubeProximityEndpointSource.js';
import { NativeSubgraphBoundaryResolver } from '../../frontend/comfyui/ui/cube/graph/NativeSubgraphBoundaryResolver.js';
import type { ComfyGraph, ComfyNode } from '../../frontend/comfyui/ui/types/graph.js';

describe('NativeCubeProximityEndpointSource', () => {
  test('discovers real root slots and recursively resolves nested Cube execution endpoints', () => {
    const producer = node('producer', [], [{ name: 'image', type: 'IMAGE', links: [1] }]);
    const outputBoundary = boundaryNode(-20);
    const outputGraph = graph([producer], [link(1, producer.id, 0, outputBoundary.id, 0)]);
    const outputCube = subgraphNode('cube-output', [], [{ name: 'image', type: 'IMAGE' }], {
      graph: outputGraph,
      inputLinks: [],
      outputLink: resolvedOutput(producer, 0),
      cubeId: 'definition-output',
      instanceId: 'instance-output',
    });

    const consumer = node('consumer', [{ name: 'image', type: 'IMAGE', link: 2 }], []);
    const nestedInputBoundary = boundaryNode(-10);
    const nestedGraph = graph([consumer], [link(2, nestedInputBoundary.id, 0, consumer.id, 0)]);
    const nested = subgraphNode('nested', [{ name: 'image', type: 'IMAGE', link: 3 }], [], {
      graph: nestedGraph,
      inputLinks: [resolvedInput(consumer, 0)],
      outputLink: undefined,
    });
    const cubeInputBoundary = boundaryNode(-10);
    const inputGraph = graph([nested], [link(3, cubeInputBoundary.id, 0, nested.id, 0)]);
    const inputCube = subgraphNode('cube-input', [{ name: 'image', type: 'IMAGE' }], [], {
      graph: inputGraph,
      inputLinks: [resolvedInput(nested, 0)],
      outputLink: undefined,
      cubeId: 'definition-input',
      instanceId: 'instance-input',
    });

    const ordinaryOutput = node(
      'ordinary-output',
      [],
      [{ name: 'mask', type: 'MASK', links: null }],
    );
    const ordinaryInput = node('ordinary-input', [{ name: 'mask', type: 'MASK', link: null }], []);
    const explicitlyLinked = node(
      'linked',
      [{ name: 'used', type: 'IMAGE', link: 99 }],
      [{ name: 'used', type: 'IMAGE', links: [99] }],
    );
    const root = graph([outputCube, inputCube, ordinaryOutput, ordinaryInput, explicitlyLinked]);
    const source = new NativeCubeProximityEndpointSource(
      console,
      new NativeSubgraphBoundaryResolver(console),
    );

    const endpoints = source.discover(root);

    expect(endpoints.outputs).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          endpointId: 'cube-output',
          cube: 'definition-output',
          instanceId: 'instance-output',
          originId: 'cube-output:producer',
          originSlot: 0,
          slotName: 'image',
          slotPos: [110, 25],
        }),
        expect.objectContaining({
          endpointId: 'ordinary-output',
          cube: null,
          originId: 'ordinary-output',
          originSlot: 0,
          slotName: 'mask',
        }),
      ]),
    );
    expect(endpoints.inputs).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          endpointId: 'cube-input',
          cube: 'definition-input',
          instanceId: 'instance-input',
          promptTargets: [
            {
              nodeId: 'cube-input:nested:consumer',
              inputSlot: 0,
              inputName: 'image',
            },
          ],
          slotName: 'image',
          slotPos: [200, 25],
        }),
        expect.objectContaining({
          endpointId: 'ordinary-input',
          cube: null,
          promptTargets: [{ nodeId: 'ordinary-input', inputSlot: 0, inputName: 'mask' }],
          slotName: 'mask',
        }),
      ]),
    );
    expect(endpoints.outputs).toHaveLength(2);
    expect(endpoints.inputs).toHaveLength(2);
  });

  test('uses Comfy unified slot geometry before the legacy connection-position fallback', () => {
    const output = node('native-output', [], [{ name: 'image', type: 'IMAGE', links: null }]);
    output.getSlotPosition = jest.fn(() => [340, 275]);
    output.getConnectionPos = jest.fn(() => [100, 25]);
    const input = node('native-input', [{ name: 'image', type: 'IMAGE', link: null }], []);
    input.getSlotPosition = jest.fn(() => [380, 290]);
    input.getConnectionPos = jest.fn(() => [0, 25]);

    const endpoints = new NativeCubeProximityEndpointSource(
      console,
      new NativeSubgraphBoundaryResolver(console),
    ).discover(graph([output, input]));

    expect(endpoints.outputs[0]?.slotPos).toEqual([340, 275]);
    expect(endpoints.inputs[0]?.slotPos).toEqual([380, 290]);
    expect(output.getConnectionPos).not.toHaveBeenCalled();
    expect(input.getConnectionPos).not.toHaveBeenCalled();
  });
});

interface SubgraphOptions {
  graph: ComfyGraph;
  inputLinks: Array<ReturnType<typeof resolvedInput>>;
  outputLink: ReturnType<typeof resolvedOutput> | undefined;
  cubeId?: string;
  instanceId?: string;
}

function subgraphNode(
  id: string,
  inputs: NonNullable<ComfyNode['inputs']>,
  outputs: NonNullable<ComfyNode['outputs']>,
  options: SubgraphOptions,
): ComfyNode {
  return {
    id,
    type: 'Subgraph',
    title: id,
    pos: id === 'cube-input' ? [200, 0] : [0, 0],
    size: [110, 50],
    inputs,
    outputs,
    properties: options.cubeId
      ? {
          sugarcubes_kind: 'cube',
          sugarcubes_cube: {
            cube_id: options.cubeId,
            instance_id: options.instanceId,
          },
        }
      : {},
    graph: null,
    subgraph: {
      ...options.graph,
      id: `${id}-definition`,
      name: id,
      inputs,
      outputs,
      inputNode: { slots: [] },
      outputNode: { slots: [] },
      add: jest.fn(),
      remove: jest.fn(),
      addInput: jest.fn(),
      addOutput: jest.fn(),
      configure: jest.fn(),
    },
    isSubgraphNode: () => true,
    resolveSubgraphInputLinks: () => options.inputLinks,
    resolveSubgraphOutputLink: () => options.outputLink,
    getConnectionPos(isInput: boolean, _slot: number, output?: Float32Array) {
      const point: [number, number] = isInput ? [200, 25] : [110, 25];
      if (output) {
        output[0] = point[0];
        output[1] = point[1];
      }
      return point;
    },
    connect: jest.fn(),
    serialize: jest.fn(),
  };
}

function node(
  id: string,
  inputs: NonNullable<ComfyNode['inputs']>,
  outputs: NonNullable<ComfyNode['outputs']>,
): ComfyNode {
  return {
    id,
    type: id,
    title: id,
    pos: [0, 0],
    size: [100, 50],
    inputs,
    outputs,
    properties: {},
    getConnectionPos(isInput: boolean, _slot: number, output?: Float32Array) {
      const point: [number, number] = isInput ? [0, 25] : [100, 25];
      if (output) {
        output[0] = point[0];
        output[1] = point[1];
      }
      return point;
    },
  };
}

function boundaryNode(id: number): ComfyNode {
  return { id, inputs: [], outputs: [], pos: [0, 0], size: [0, 0] };
}

function graph(nodes: ComfyNode[], links: ReturnType<typeof link>[] = []): ComfyGraph {
  const byId = new Map(nodes.map((entry) => [String(entry.id), entry]));
  return {
    _nodes: nodes,
    _links: new Map(links.map((entry) => [entry.id ?? 0, entry])),
    getLink(id) {
      return links.find((entry) => String(entry.id) === String(id)) ?? null;
    },
    getNodeById(id: string | number) {
      return byId.get(String(id));
    },
  };
}

function link(
  id: number,
  originId: string | number | undefined,
  originSlot: number,
  targetId: string | number | undefined,
  targetSlot: number,
) {
  return {
    id,
    origin_id: originId ?? null,
    origin_slot: originSlot,
    target_id: targetId ?? null,
    target_slot: targetSlot,
  };
}

function resolvedInput(inputNode: ComfyNode, inputSlot: number) {
  return {
    link: link(1, -10, 0, inputNode.id, inputSlot),
    inputNode,
    input: inputNode.inputs?.[inputSlot],
  };
}

function resolvedOutput(outputNode: ComfyNode, outputSlot: number) {
  return {
    link: link(1, outputNode.id, outputSlot, -20, 0),
    outputNode,
    output: outputNode.outputs?.[outputSlot],
  };
}
