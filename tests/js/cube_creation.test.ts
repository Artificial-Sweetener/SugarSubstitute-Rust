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
import { describe, expect, test, beforeEach } from '@jest/globals';
import type {
  ComfyGraph,
  ComfyInput,
  ComfyLink,
  ComfyNode,
  ComfyOutput,
  ComfyWidget,
} from '../../web/comfyui/ui/types/graph.js';

type CubeCreationModule = typeof import('../../web/js/cube_creation.js');
let createCubeFromSelection: CubeCreationModule['createCubeFromSelection'];
let wrapMarkerToCube: CubeCreationModule['wrapMarkerToCube'];

interface TestNode extends ComfyNode {
  id: number;
  type: string;
  pos: number[];
  size: number[];
  inputs: ComfyInput[];
  outputs: ComfyOutput[];
  widgets: ComfyWidget[];
  graph?: TestGraph;
  connect(outSlot: number, target: TestNode, inSlot: number): ComfyLink | undefined;
  disconnectOutput(outSlot: number, target: TestNode): void;
  disconnectInput(inSlot: number): void;
}

interface TestGraph extends ComfyGraph {
  _nodes: TestNode[];
  links: ComfyLink[];
  add(node: TestNode): TestNode;
  removeLink(id: string | number): void;
  connect?(fromNode: TestNode, fromSlot: number, toNode: TestNode, toSlot: number): ComfyLink;
}

function makeGraph() {
  let linkId = 1;
  const graph: TestGraph = {
    _nodes: [] as TestNode[],
    links: [] as ComfyLink[],
    add(node: TestNode) {
      node.graph = this;
      this._nodes.push(node);
      return node;
    },
    removeLink(id: string | number) {
      this.links = this.links.filter((link) => link && link.id !== id);
    },
  };

  function connect(fromNode: TestNode, fromSlot: number, toNode: TestNode, toSlot: number) {
    const link = {
      id: linkId++,
      origin_id: fromNode.id,
      origin_slot: fromSlot,
      target_id: toNode.id,
      target_slot: toSlot,
      type: '*',
    };
    graph.links.push(link);
    if (toNode.inputs && toNode.inputs[toSlot]) {
      toNode.inputs[toSlot].link = link.id;
    }
    if (fromNode.outputs && fromNode.outputs[fromSlot]) {
      const output = fromNode.outputs[fromSlot];
      output.links = output.links || [];
      output.links.push(link.id);
    }
    return link;
  }

  graph.connect = connect;

  function makeNode(id: number, type: string): TestNode {
    const node: TestNode = {
      id,
      type,
      pos: [id * 10, 0],
      size: [100, 50],
      inputs: [{ name: 'value', link: null }],
      outputs: [{ name: 'value', links: [] }],
      widgets: [],
      connect(outSlot: number, target: TestNode, inSlot: number) {
        return connect(node, outSlot, target, inSlot);
      },
      disconnectOutput(outSlot: number, target: TestNode) {
        graph.links = graph.links.filter(
          (link) =>
            !(
              link.origin_id === node.id &&
              link.target_id === target.id &&
              link.origin_slot === outSlot
            ),
        );
      },
      disconnectInput(inSlot: number) {
        if (!node.inputs[inSlot]) return;
        node.inputs[inSlot].link = null;
      },
    };
    graph.add(node);
    return node;
  }

  return { graph, makeNode, connect };
}

beforeEach(async () => {
  globalThis.LiteGraph = {
    createNode(type: string) {
      const node: TestNode = {
        id: Math.floor(Math.random() * 10000) + 100,
        type,
        pos: [0, 0],
        size: [140, 46],
        inputs: [{ name: 'value', link: null }],
        outputs: [{ name: 'value', links: [] }],
        widgets: [
          { name: 'cube_id', value: '' },
          { name: 'default_alias', value: '' },
          { name: 'key', value: '' },
        ],
        disconnectOutput: () => undefined,
        disconnectInput: () => undefined,
        connect(outSlot: number, target: TestNode, inSlot: number) {
          return this.graph?.connect?.(this, outSlot, target, inSlot);
        },
      };
      return node;
    },
  } as unknown as LiteGraphHost;
  globalThis.window = {} as Window & typeof globalThis;
  const module = await import('../../web/js/cube_creation.js');
  createCubeFromSelection = module.createCubeFromSelection;
  wrapMarkerToCube = module.wrapMarkerToCube;
});

describe('cube creation', () => {
  test('createCubeFromSelection validates inputs', () => {
    expect(createCubeFromSelection({ graph: null, defaultAlias: 'Demo', selection: [] })).toEqual({
      ok: false,
      message: 'Graph unavailable.',
    });
    const { graph } = makeGraph();
    expect(createCubeFromSelection({ graph, defaultAlias: '', selection: [] }).ok).toBe(false);
    expect(createCubeFromSelection({ graph, defaultAlias: 'Demo', selection: [] }).ok).toBe(false);
  });

  test('createCubeFromSelection inserts markers on boundary links', () => {
    const { graph, makeNode, connect } = makeGraph();
    const source = makeNode(1, 'Source');
    const inside = makeNode(2, 'Inside');
    const sink = makeNode(3, 'Sink');
    connect(source, 0, inside, 0);
    connect(inside, 0, sink, 0);

    const result = createCubeFromSelection({
      graph,
      defaultAlias: 'Demo',
      selection: [inside],
    });

    expect(result.ok).toBe(true);
    expect(result.markers).toHaveLength(2);
    expect(graph.links).toHaveLength(4);
    const markerTypes = result.markers!.map((node) => node.type);
    expect(markerTypes).toContain('SugarCubes.CubeInput');
    expect(markerTypes).toContain('SugarCubes.CubeOutput');
  });

  test('wrapMarkerToCube rejects mismatched marker names', () => {
    const { graph, makeNode, connect } = makeGraph();
    const marker = makeNode(10, 'SugarCubes.CubeInput');
    marker.widgets.push({ name: 'default_alias', value: 'Alpha' });
    const other = makeNode(11, 'SugarCubes.CubeOutput');
    other.widgets.push({ name: 'default_alias', value: 'Beta' });
    const node = makeNode(12, 'Node');
    const outside = makeNode(13, 'Outside');
    connect(marker, 0, node, 0);
    connect(outside, 0, node, 0);
    connect(node, 0, other, 0);

    const result = wrapMarkerToCube(marker, { defaultAlias: 'Alpha' });
    expect(result.ok).toBe(false);
    expect(result.message).toMatch(/Multiple default aliases/);
  });
});
