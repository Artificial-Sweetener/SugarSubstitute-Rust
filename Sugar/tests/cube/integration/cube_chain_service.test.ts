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
/** Prove the stack and the graph are one representation: each change in one is the other's. */
import { expect, test } from '@jest/globals';
import { CubeChainService } from '../../../frontend/comfyui/ui/cube/chain/CubeChainService.js';
import {
  ComfyCubeChainCommitHost,
  type CubeChainCreation,
} from '../../../frontend/comfyui/ui/cube/chain/ComfyCubeChainCommitHost.js';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { ComfyLink, GraphId } from '../../../frontend/comfyui/ui/types/graph.js';

test('two Cubes added in masonry sit in the graph exactly as the add button puts them', () => {
  const dialog = workflow();
  dialog.chain.apply([creation('t2i', 836), creation('upscale', 1120)]);
  const button = workflow();
  button.chain.apply([creation('t2i', 836)]);
  button.chain.insertAfter('t2i', { ...creation('upscale', 1120) });
  for (const id of ['t2i', 'upscale'])
    expect([...button.node(id).pos]).toEqual([...dialog.node(id).pos]);
  expect([...dialog.node('upscale').pos]).toEqual([860, 0]);
  expect(dialog.chain.order()).toEqual(['t2i', 'upscale']);
});

test('wiring Cubes on the graph into another order changes the stack', () => {
  const w = workflow();
  w.chain.apply([creation('a', 400), creation('b', 400)]);
  expect(w.chain.order()).toEqual(['a', 'b']);
  w.connect('b', 'a');
  expect(w.chain.order()).toEqual(['b', 'a']);
});

test('moving a floating Cube in the stack moves it in the graph', () => {
  const w = workflow();
  w.chain.apply([creation('a', 400), creation('b', 200)]);
  w.chain.move('b', -1);
  expect([...w.node('b').pos]).toEqual([0, 0]);
  expect([...w.node('a').pos]).toEqual([224, 0]);
  expect(w.chain.order()).toEqual(['b', 'a']);
});

test('moving a hardwired Cube rewires it, and a move its wiring cannot survive is refused', () => {
  const w = workflow();
  w.chain.apply([creation('a', 400), creation('b', 400), creation('c', 400)]);
  w.connect('a', 'b');
  w.connect('b', 'c');
  expect(w.chain.canMove('b', -1)).toBe(false);
  expect(() => w.chain.move('b', -1)).toThrow('cannot be first');
  expect(w.chain.order()).toEqual(['a', 'b', 'c']);
  w.chain.move('c', -1);
  expect(w.chain.order()).toEqual(['a', 'c', 'b']);
  expect(w.sourceOf('c')).toBe('a');
  expect(w.sourceOf('b')).toBe('c');
  expect([...w.node('c').pos]).toEqual([424, 0]);
});

/** A new Cube the chain should create. */
function creation(id: string, width: number): { source: 'new' } & CubeChainCreation {
  return { source: 'new', instanceId: id, alias: id, payload: { id }, size: [width, 600] };
}

/** An empty root workflow with a real chain, commit host and link bookkeeping. */
function workflow() {
  const root = new Map<GraphId, CubeNode>();
  const links = new Map<GraphId, ComfyLink>();
  let nextLink = 1;
  const nodes = new CubeNodeCatalog();
  const graph = {
    get _nodes() {
      return [...root.values()];
    },
    getLink: (id: GraphId) => links.get(id) ?? null,
  };
  const build = (
    payload: unknown,
    options: { instanceId?: string; position?: number[]; size?: number[] } = {},
  ) => {
    const id = String((payload as { id: string }).id ?? options.instanceId);
    const node = cube(id, options.size?.[0] ?? 400);
    node.connect = (sourceSlot: number, target: unknown, targetSlot: number) => {
      const to = target as CubeNode;
      const linkId = nextLink++;
      links.set(linkId, {
        id: linkId,
        origin_id: node.id,
        origin_slot: sourceSlot,
        target_id: to.id,
        target_slot: targetSlot,
      });
      const input = to.inputs[targetSlot];
      if (input) input.link = linkId;
      return {};
    };
    Object.assign(node, {
      disconnectInput: (slot: number) => {
        const input = node.inputs[slot];
        if (input?.link !== null && input?.link !== undefined) links.delete(input.link);
        if (input) input.link = null;
      },
    });
    return { node, subgraph: node.subgraph, warnings: [], internalNodeCount: 0 };
  };
  const chain = new CubeChainService({
    graph,
    nodes,
    isCurrentRoot: () => true,
    matches: () => [],
    accepts: (output, input) => output === input,
    commit: new ComfyCubeChainCommitHost({
      graph: {
        add: (node) => {
          root.set(node.id, node);
        },
        remove: (node) => {
          root.delete(node.id);
        },
        getNodeById: (id) => root.get(id),
        getLink: (id) => links.get(id) ?? null,
      },
      nodes,
      history: { beforeChange() {}, afterChange() {}, setDirtyCanvas() {} },
      construction: { construct: build, discard: () => undefined },
      definitions: {
        stage: (payload) => ({ payload, definitionIds: [] }),
        discard: () => undefined,
      },
      reconcile: () => undefined,
      announce: () => undefined,
    }),
  });
  const node = (id: string): CubeNode => {
    const found = root.get(id);
    if (!found) throw new Error(`No Cube ${id}`);
    return found;
  };
  return {
    chain,
    node,
    connect: (source: string, target: string) => node(source).connect(0, node(target), 0),
    sourceOf: (target: string) => {
      const id = node(target).inputs[0]?.link;
      return id === null || id === undefined ? null : String(links.get(id)?.origin_id);
    },
  };
}

/** A Cube with one image port each way. */
function cube(id: string, width: number): CubeNode {
  const subgraph: NativeCubeSubgraph = {
    id: `definition-${id}`,
    name: id,
    _nodes: [],
    inputs: [],
    outputs: [],
    inputNode: {},
    outputNode: {},
    add() {},
    remove() {},
    addInput() {
      return { connect() {} };
    },
    addOutput() {
      return { connect() {} };
    },
    configure() {},
  };
  return {
    id,
    title: id,
    pos: [0, 0],
    size: [width, 600],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: id, instance_alias: id },
    },
    inputs: [{ name: 'image', type: 'IMAGE', link: null }],
    outputs: [{ name: 'image', type: 'IMAGE', links: [] }],
    subgraph,
    isSubgraphNode: () => true,
    serialize: () => ({}),
    connect() {},
  };
}
