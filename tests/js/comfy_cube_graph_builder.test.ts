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
  test('seeds empty authoring drafts with unbound native editor boundaries', () => {
    const inputNode = {
      arrange: jest.fn(),
      bounding: [0, 0, 75, 100],
      emptySlot: { name: '' },
    };
    const outputNode = {
      arrange: jest.fn(),
      bounding: [0, 0, 75, 100],
      emptySlot: { name: '' },
    };
    const subgraph = {
      addInput: jest.fn(),
      addOutput: jest.fn(),
      inputNode,
      outputNode,
    } as unknown as NativeCubeSubgraph;
    const createSubgraph = jest.fn(() => subgraph);
    const builder = new ComfyCubeGraphBuilder({
      rootGraph: { createSubgraph },
      createNode: () => null,
      createUuid: () => 'draft-definition',
    });

    expect(builder.createEmptyDraft('Cube: Untitled Cube')).toBe(subgraph);
    expect(createSubgraph).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'draft-definition',
        name: 'Cube: Untitled Cube',
        inputs: [],
        outputs: [],
        inputNode: { id: -10, bounding: [120, 200, 75, 100] },
        outputNode: { id: -20, bounding: [780, 200, 75, 100] },
      }),
    );
    expect(subgraph.addInput).not.toHaveBeenCalled();
    expect(subgraph.addOutput).not.toHaveBeenCalled();
    expect(inputNode.emptySlot.name).toBe('Add Input');
    expect(outputNode.emptySlot.name).toBe('Add Output');
    expect(inputNode.arrange).toHaveBeenCalledTimes(1);
    expect(outputNode.arrange).toHaveBeenCalledTimes(1);
    expect(inputNode.bounding).toEqual([120, 200, 75, 100]);
    expect(outputNode.bounding).toEqual([780, 200, 75, 100]);
  });

  test('keeps imported Cube definitions free of authoring placeholder boundaries', () => {
    const subgraph = {} as NativeCubeSubgraph;
    const createSubgraph = jest.fn(() => subgraph);
    const builder = new ComfyCubeGraphBuilder({
      rootGraph: { createSubgraph },
      createNode: () => null,
      createUuid: () => 'definition',
    });

    expect(builder.createEmptyDefinition('Cube: Imported')).toBe(subgraph);
    expect(createSubgraph).toHaveBeenCalledWith(
      expect.objectContaining({ inputs: [], outputs: [] }),
    );
  });

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
    expect(subgraph.addOutput).toHaveBeenCalledWith('value', 'IMAGE');
    expect(inputConnect).toHaveBeenCalledWith(internalNodes[0]?.inputs[0], internalNodes[0]);
    expect(outputConnect).toHaveBeenCalledWith(internalNodes[0]?.outputs[0], internalNodes[0]);
  });

  test('positions native boundaries outside their connected authored node geometry', () => {
    const internalNodes: NativeGraphNode[] = [];
    const inputNode = {
      bounding: [0, 0, 100, 100],
      boundingRect: [0, 0, 100, 100],
      pos: [0, 0],
      size: [100, 100],
      arrange: jest.fn(),
    };
    const outputNode = {
      bounding: [0, 0, 100, 100],
      boundingRect: [0, 0, 100, 100],
      pos: [0, 0],
      size: [100, 100],
      arrange: jest.fn(),
    };
    const subgraph = {
      id: 'cube-definition',
      name: 'Cube',
      _nodes: internalNodes,
      inputNode,
      outputNode,
      add: (node: NativeGraphNode) => internalNodes.push(node),
      addInput: jest.fn(() => ({ connect: jest.fn() })),
      addOutput: jest.fn(() => ({ connect: jest.fn() })),
    } as unknown as NativeCubeSubgraph;
    const builder = new ComfyCubeGraphBuilder({
      rootGraph: { createSubgraph: () => subgraph },
      createNode: (type) => makeNode(type),
      createUuid: () => 'node-id',
    });

    builder.build(
      {
        layout: {
          origin: [100, 200],
          groups: [
            {
              sugarcubes: {
                managed: true,
                authored_layout: {
                  schema: 1,
                  origin: [100, 200],
                  entries: {
                    'input.value': {
                      x: -20,
                      y: 30,
                      w: 140,
                      h: 46,
                      collapsed: false,
                      title: 'IMAGE Input',
                    },
                    node: {
                      x: 30,
                      y: 40,
                      w: 220,
                      h: 160,
                      collapsed: false,
                      title: 'Example',
                    },
                    'output.value': {
                      x: 500,
                      y: 60,
                      w: 140,
                      h: 46,
                      collapsed: false,
                      title: 'IMAGE Output',
                    },
                  },
                  group: null,
                },
              },
            },
          ],
        },
        nodes: [
          {
            symbol: 'node',
            class_type: 'Example',
            inputs: {},
            layout: { pos: [130, 240], size: [220, 160] },
          },
        ],
        markers: [
          {
            alias: 'input.value',
            kind: 'input',
            layout: { pos: [80, 230], size: [140, 46] },
          },
          {
            alias: 'output.value',
            kind: 'output',
            layout: { pos: [600, 260], size: [140, 46] },
          },
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
      },
      'Positioned Cube',
    );

    expect(inputNode.arrange).toHaveBeenCalledTimes(2);
    expect(outputNode.arrange).toHaveBeenCalledTimes(2);
    expect(inputNode.pos).toEqual([-110, 70]);
    expect(outputNode.pos).toEqual([290, 70]);
    expect(inputNode.bounding.slice(0, 2)).toEqual([-110, 70]);
    expect(outputNode.bounding.slice(0, 2)).toEqual([290, 70]);
    expect(inputNode.boundingRect.slice(0, 2)).toEqual([-110, 70]);
    expect(outputNode.boundingRect.slice(0, 2)).toEqual([290, 70]);
  });

  test('reapplies authored node geometry after graph insertion and widget restoration', () => {
    const internalNodes: NativeGraphNode[] = [];
    const node = makeNode('Resizable');
    node.widgets = [
      {
        name: 'value',
        value: 0,
        callback: () => {
          node.size[0] = 120;
          node.size[1] = 90;
        },
      },
    ];
    const subgraph = {
      id: 'cube-definition',
      name: 'Cube',
      _nodes: internalNodes,
      inputNode: {},
      outputNode: {},
      add: (entry: NativeGraphNode) => {
        internalNodes.push(entry);
        entry.size[0] = 100;
        entry.size[1] = 80;
      },
      addInput: jest.fn(),
      addOutput: jest.fn(),
    } as unknown as NativeCubeSubgraph;
    const builder = new ComfyCubeGraphBuilder({
      rootGraph: { createSubgraph: () => subgraph },
      createNode: () => node,
      createUuid: () => 'node-id',
    });

    builder.build(
      {
        layout: { origin: [100, 200] },
        nodes: [
          {
            symbol: 'node',
            class_type: 'Resizable',
            inputs: {},
            extras: { widgets_values: [7] },
            layout: { pos: [140, 260], size: [280, 190] },
          },
        ],
        markers: [],
        connections: [],
      },
      'Resizable Cube',
    );

    expect(node.pos).toEqual([40, 60]);
    expect(node.size).toEqual([280, 190]);
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
    nested.configure = jest.fn();
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
    expect(nested.configure).toHaveBeenCalledWith({});
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
    expect(subgraph.addOutput).toHaveBeenCalledWith('image', 'IMAGE');
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
