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
/** Verify saved Cube output markers retain their execution semantics after native loading. */

import { CubeOutputPromptAdapter } from '../../frontend/comfyui/ui/cube/execution/CubeOutputPromptAdapter.js';
import { buildCubeOutputExecutionId } from '../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionIdentity.js';
import { NativeSubgraphBoundaryResolver } from '../../frontend/comfyui/ui/cube/graph/NativeSubgraphBoundaryResolver.js';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { isRecord } from '../../frontend/comfyui/ui/types/common.js';
import type { UnknownRecord } from '../../frontend/comfyui/ui/types/common.js';
import type { ComfyNode } from '../../frontend/comfyui/ui/types/graph.js';

describe('CubeOutputPromptAdapter', () => {
  test('restores one execution-only CubeOutput sink without mutating Comfy workflow data', () => {
    const producer = leaf('decode');
    const cube = cubeNode('cube-instance', [producer], ['output.image'], producer);
    const workflow = { nodes: [{ id: cube.id }] };
    const original = {
      workflow,
      output: {
        'cube-instance:decode': {
          inputs: {},
          class_type: 'VAEDecode',
          _meta: { title: 'VAE Decode' },
        },
      },
    };
    const adapter = outputAdapter([cube]);

    const transformed = requirePrompt(adapter.apply(original));

    expect(transformed).not.toBe(original);
    expect(transformed.workflow).toBe(workflow);
    expect(transformed.output).not.toBe(original.output);
    expect(original.output).not.toHaveProperty(buildCubeOutputExecutionId(cube.id, 0));
    expect(transformed.output[buildCubeOutputExecutionId(cube.id, 0)]).toEqual({
      inputs: {
        value: ['cube-instance:decode', 0],
        cube_id: 'Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube',
        default_alias: 'SDXL/Text to Image',
        instance_alias: 'My SDXL Cube',
        instance_id: 'stable-instance',
      },
      class_type: 'SugarCubes.CubeOutput',
      _meta: { title: 'output.image' },
    });
  });

  test('keeps a sink for an externally connected output and resolves nested execution paths', () => {
    const producer = leaf('sampler');
    const nested = subgraphHost('nested', [producer], producer);
    const cube = cubeNode('cube', [nested], ['output.image'], nested);
    cube.outputs = [{ name: 'output.image', type: 'IMAGE', links: ['external-link'] }];
    const adapter = outputAdapter([cube]);

    const transformed = requirePrompt(
      adapter.apply({
        output: {
          'cube:nested:sampler': { inputs: {}, class_type: 'KSampler' },
          preview: {
            inputs: { images: ['cube:nested:sampler', 0] },
            class_type: 'PreviewImage',
          },
        },
      }),
    );

    expect(transformed.output[buildCubeOutputExecutionId(cube.id, 0)]).toMatchObject({
      inputs: { value: ['cube:nested:sampler', 0] },
      class_type: 'SugarCubes.CubeOutput',
    });
  });

  test('adds one independently addressable sink for every ordered Cube output', () => {
    const first = leaf('first');
    const second = leaf('second');
    const cube = cubeNode('cube', [first, second], ['output.image', 'output.mask'], first);
    cube.resolveSubgraphOutputLink = (slot: number) => resolvedOutput(slot === 0 ? first : second);
    const adapter = outputAdapter([cube]);

    const transformed = requirePrompt(adapter.apply({ output: {} }));

    expect(Object.keys(transformed.output)).toEqual([
      buildCubeOutputExecutionId(cube.id, 0),
      buildCubeOutputExecutionId(cube.id, 1),
    ]);
    expect(transformed.output[buildCubeOutputExecutionId(cube.id, 1)]).toMatchObject({
      inputs: { value: ['cube:second', 0] },
      _meta: { title: 'output.mask' },
    });
  });

  test('returns unrelated payloads unchanged and fails closed on prompt identifier collisions', () => {
    const cube = cubeNode('cube', [leaf('decode')], ['output.image'], leaf('decode'));
    const adapter = outputAdapter([cube]);
    const unrelated = { workflow: {} };

    expect(adapter.apply(unrelated)).toBe(unrelated);
    expect(() =>
      adapter.apply({
        output: {
          [buildCubeOutputExecutionId(cube.id, 0)]: {
            inputs: {},
            class_type: 'UnrelatedNode',
          },
        },
      }),
    ).toThrow('collides');
  });
});

function outputAdapter(cubes: readonly CubeNode[]): CubeOutputPromptAdapter {
  return new CubeOutputPromptAdapter({
    getCubes: () => cubes,
    boundaryResolver: new NativeSubgraphBoundaryResolver(console),
  });
}

function cubeNode(
  id: string,
  nodes: ComfyNode[],
  outputNames: string[],
  outputNode: ComfyNode,
): CubeNode {
  const subgraph = nativeSubgraph(`${id}-definition`, nodes, outputNames);
  return {
    id,
    type: subgraph.id,
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        cube_id: 'Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube',
        default_alias: 'SDXL/Text to Image',
        instance_alias: 'My SDXL Cube',
        instance_id: 'stable-instance',
      },
    },
    inputs: [],
    outputs: outputNames.map((name) => ({ name, type: 'IMAGE', links: null })),
    subgraph,
    isSubgraphNode: () => true,
    resolveSubgraphOutputLink: () => resolvedOutput(outputNode),
    connect() {},
    serialize: () => ({}),
  };
}

function subgraphHost(id: string, nodes: ComfyNode[], outputNode: ComfyNode): ComfyNode {
  return {
    id,
    type: `${id}-definition`,
    pos: [0, 0],
    size: [100, 100],
    properties: {},
    inputs: [],
    outputs: [{ name: 'image', type: 'IMAGE', links: null }],
    subgraph: nativeSubgraph(`${id}-definition`, nodes, ['image']),
    isSubgraphNode: () => true,
    resolveSubgraphOutputLink: () => resolvedOutput(outputNode),
  };
}

function nativeSubgraph(id: string, nodes: ComfyNode[], outputNames: string[]): NativeCubeSubgraph {
  return {
    id,
    name: id,
    _nodes: nodes,
    inputs: [],
    outputs: outputNames.map((name) => ({ name, type: 'IMAGE' })),
    inputNode: {},
    outputNode: {},
    add() {},
    remove() {},
    addInput() {
      throw new Error('not used');
    },
    addOutput() {
      throw new Error('not used');
    },
    configure() {},
  } as unknown as NativeCubeSubgraph;
}

function leaf(id: string): ComfyNode {
  return {
    id,
    type: id,
    title: id,
    pos: [0, 0],
    size: [100, 100],
    properties: {},
    inputs: [],
    outputs: [{ name: 'image', type: 'IMAGE', links: null }],
  };
}

function resolvedOutput(outputNode: ComfyNode) {
  return {
    link: {
      id: 1,
      origin_id: outputNode.id ?? null,
      origin_slot: 0,
      target_id: -20,
      target_slot: 0,
    },
    outputNode,
    output: outputNode.outputs?.[0],
  };
}

interface PromptEnvelope {
  workflow?: unknown;
  output: UnknownRecord;
}

function requirePrompt(value: unknown): PromptEnvelope {
  if (!isRecord(value) || !isRecord(value.output)) {
    throw new TypeError('Expected a prompt envelope.');
  }
  return {
    ...(value.workflow === undefined ? {} : { workflow: value.workflow }),
    output: value.output,
  };
}
