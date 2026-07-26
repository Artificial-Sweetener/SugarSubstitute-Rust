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
/** Characterize native Cube graph assembly and root isolation. */

import { jest } from '@jest/globals';
import {
  ComfyCubeGraphBuilder,
  type NativeCubeSubgraph,
  type NativeGraphNode,
} from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { ImportPayload } from '../../frontend/comfyui/ui/import/PlacementPayload.js';
import type { ComfyInput, ComfyOutput } from '../../frontend/comfyui/ui/types/graph.js';

function makeNode(type: string): NativeGraphNode {
  const input: ComfyInput = { name: 'value', type: 'IMAGE', link: null };
  const output: ComfyOutput = { name: 'IMAGE', type: 'IMAGE', links: null };
  return {
    id: -1,
    type,
    title: type,
    pos: [0, 0],
    size: [100, 80],
    inputs: [input],
    outputs: [output],
    widgets: [],
    properties: {},
    connect: jest.fn(),
  };
}

describe('ComfyCubeGraphBuilder', () => {
  test('places real nodes only in the native subgraph and owns boundary I/O there', () => {
    const rootNodes: NativeGraphNode[] = [];
    const internalNodes: NativeGraphNode[] = [];
    const inputConnect = jest.fn();
    const outputConnect = jest.fn();
    const subgraph = {
      id: 'cube-definition',
      name: 'Cube',
      _nodes: internalNodes,
      inputNode: { arrange: jest.fn() },
      outputNode: { arrange: jest.fn() },
      add: (node: NativeGraphNode) => internalNodes.push(node),
      addInput: jest.fn(() => ({ connect: inputConnect })),
      addOutput: jest.fn(() => ({ connect: outputConnect })),
    } as unknown as NativeCubeSubgraph;
    let uuid = 0;
    const builder = new ComfyCubeGraphBuilder({
      rootGraph: { createSubgraph: jest.fn(() => subgraph) },
      createNode: (type) => makeNode(type),
      createUuid: () => `uuid-${++uuid}`,
    });
    const payload: ImportPayload = {
      layout: { origin: [100, 200] },
      nodes: [
        {
          symbol: 'node',
          class_type: 'Example',
          inputs: {},
          layout: { pos: [130, 240], size: [220, 160] },
        },
      ],
      markers: [
        { alias: 'input.value', kind: 'input' },
        { alias: 'output.value', kind: 'output' },
      ],
      connections: [
        {
          from: { symbol: 'input.value', slot: 0 },
          to: { symbol: 'node', input: 'value' },
        },
        {
          from: { symbol: 'node', slot: 0 },
          to: { symbol: 'output.value', input: 'value' },
        },
      ],
    };

    const built = builder.build(payload, 'Example Cube');

    expect(rootNodes).toHaveLength(0);
    expect(built.subgraph).toBe(subgraph);
    expect(internalNodes).toHaveLength(1);
    expect(internalNodes[0]?.pos).toEqual([30, 40]);
    expect(internalNodes[0]?.size).toEqual([220, 160]);
    expect(subgraph.addInput).toHaveBeenCalledWith('input.value', 'IMAGE');
    expect(subgraph.addOutput).toHaveBeenCalledWith('output.value', 'IMAGE');
    expect(inputConnect).toHaveBeenCalledWith(internalNodes[0]?.inputs[0], internalNodes[0]);
    expect(outputConnect).toHaveBeenCalledWith(internalNodes[0]?.outputs[0], internalNodes[0]);
  });

  test('keeps nested native subgraph nodes as real internal nodes', () => {
    const internalNodes: NativeGraphNode[] = [];
    const subgraph = {
      id: 'cube-definition',
      name: 'Cube',
      _nodes: internalNodes,
      inputNode: {},
      outputNode: {},
      add: (node: NativeGraphNode) => internalNodes.push(node),
      addInput: jest.fn(),
      addOutput: jest.fn(),
    } as unknown as NativeCubeSubgraph;
    const nested = makeNode('nested-subgraph-id');
    nested.isSubgraphNode = () => true;
    const builder = new ComfyCubeGraphBuilder({
      rootGraph: { createSubgraph: () => subgraph },
      createNode: () => nested,
      createUuid: () => 'nested-node-id',
    });

    builder.build(
      {
        nodes: [{ symbol: 'nested', class_type: 'nested-subgraph-id', inputs: {} }],
        markers: [],
        connections: [],
      },
      'Nested Cube',
    );

    expect(internalNodes).toEqual([nested]);
    expect(internalNodes[0]?.isSubgraphNode?.()).toBe(true);
  });

  test('uses the persisted nested-boundary type instead of a transient wildcard slot', () => {
    const internalNodes: NativeGraphNode[] = [];
    const inputConnect = jest.fn();
    const outputConnect = jest.fn();
    const subgraph = {
      id: 'cube-definition',
      name: 'Cube',
      _nodes: internalNodes,
      inputNode: {},
      outputNode: {},
      add: (node: NativeGraphNode) => internalNodes.push(node),
      addInput: jest.fn(() => ({ connect: inputConnect })),
      addOutput: jest.fn(() => ({ connect: outputConnect })),
    } as unknown as NativeCubeSubgraph;
    const nested = makeNode('nested-subgraph-id');
    nested.inputs[0] = { name: 'image', type: '*', link: null };
    const builder = new ComfyCubeGraphBuilder({
      rootGraph: { createSubgraph: () => subgraph },
      createNode: () => nested,
      createUuid: () => 'nested-node-id',
    });

    builder.build(
      {
        nodes: [{ symbol: 'nested', class_type: 'nested-subgraph-id', inputs: {} }],
        boundaries: {
          inputs: [
            {
              id: 'input.value',
              name: 'input.value',
              label: 'IMAGE Input',
              type: 'IMAGE',
              targets: [{ symbol: 'nested', input: 'image' }],
            },
          ],
          outputs: [
            {
              id: 'output.image',
              name: 'output.image',
              label: 'IMAGE Output',
              type: 'IMAGE',
              source: { symbol: 'nested', slot: 0 },
            },
          ],
        },
        markers: [],
        connections: [],
      },
      'Nested Cube',
    );

    expect(subgraph.addInput).toHaveBeenCalledWith('input.value', 'IMAGE');
    expect(subgraph.addOutput).toHaveBeenCalledWith('output.image', 'IMAGE');
    expect(inputConnect).toHaveBeenCalledWith(nested.inputs[0], nested);
    expect(outputConnect).toHaveBeenCalledWith(nested.outputs[0], nested);
  });

  test('restores authored values, execution mode, identity, and editor presentation', () => {
    const internalNodes: NativeGraphNode[] = [];
    const subgraph = {
      id: 'cube-definition',
      name: 'Cube',
      _nodes: internalNodes,
      inputNode: {},
      outputNode: {},
      add: (node: NativeGraphNode) => internalNodes.push(node),
      addInput: jest.fn(),
      addOutput: jest.fn(),
    } as unknown as NativeCubeSubgraph;
    const checkpoint = makeNode('CheckpointLoaderSimple');
    checkpoint.widgets = [{ name: 'ckpt_name', value: 'local-default.safetensors' }];
    const prompt = makeNode('PrimitiveStringMultiline');
    prompt.widgets = [{ name: 'value', value: 'local prompt' }];
    const builder = new ComfyCubeGraphBuilder({
      rootGraph: { createSubgraph: () => subgraph },
      createNode: (type) => (type === 'CheckpointLoaderSimple' ? checkpoint : prompt),
      createUuid: () => `node-${internalNodes.length}`,
    });

    builder.build(
      {
        layout: { origin: [100, 100] },
        nodes: [
          {
            symbol: 'checkpoint',
            class_type: 'CheckpointLoaderSimple',
            inputs: { ckpt_name: 'authored.safetensors' },
            mode: 4,
            layout: {
              pos: [120, 140],
              size: [240, 100],
              flags: { collapsed: true },
              style: { color: '#123456', bgcolor: '#654321', shape: 2 },
            },
          },
          {
            symbol: 'prompt',
            class_type: 'PrimitiveStringMultiline',
            inputs: { value: '' },
            layout: { pos: [400, 140], size: [240, 100] },
          },
        ],
        markers: [],
        connections: [],
      },
      'Configured Cube',
    );

    expect(checkpoint.widgets[0]?.value).toBe('authored.safetensors');
    expect(prompt.widgets[0]?.value).toBe('');
    expect(checkpoint.mode).toBe(4);
    expect(checkpoint.properties.sugarcubes_symbol).toBe('checkpoint');
    expect(prompt.properties.sugarcubes_symbol).toBe('prompt');
    expect(checkpoint.flags).toEqual({ collapsed: true });
    expect(checkpoint.color).toBe('#123456');
    expect(checkpoint.bgcolor).toBe('#654321');
    expect(checkpoint.shape).toBe(2);
  });

  test('leaves absent picker values at the installed node default', () => {
    const internalNodes: NativeGraphNode[] = [];
    const subgraph = {
      id: 'cube-definition',
      name: 'Cube',
      _nodes: internalNodes,
      inputNode: {},
      outputNode: {},
      add: (node: NativeGraphNode) => internalNodes.push(node),
      addInput: jest.fn(),
      addOutput: jest.fn(),
    } as unknown as NativeCubeSubgraph;
    const checkpoint = makeNode('CheckpointLoaderSimple');
    checkpoint.widgets = [{ name: 'ckpt_name', value: 'local-default.safetensors' }];
    const builder = new ComfyCubeGraphBuilder({
      rootGraph: { createSubgraph: () => subgraph },
      createNode: () => checkpoint,
      createUuid: () => 'checkpoint-node',
    });

    builder.build(
      {
        nodes: [
          {
            symbol: 'checkpoint',
            class_type: 'CheckpointLoaderSimple',
            inputs: {},
          },
        ],
        markers: [],
        connections: [],
      },
      'Defaults Cube',
    );

    expect(checkpoint.widgets[0]?.value).toBe('local-default.safetensors');
  });
});
