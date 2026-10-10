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
/** Prove compensation restores endpoint semantics once and reports native connection failure. */
import { expect, jest, test } from '@jest/globals';
import {
  captureNativeCubeConnections,
  restoreNativeCubeConnections,
} from '../../../frontend/comfyui/ui/cube/graph/NativeCubeConnections.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { ComfyLink, GraphId } from '../../../frontend/comfyui/ui/types/graph.js';

test('captures a link between two removed Cubes once and reconnects original endpoint objects and slots', () => {
  const f = fixture();
  const connections = captureNativeCubeConnections(f.graph, [f.source, f.target]);
  expect(connections).toHaveLength(1);
  expect(connections[0]?.source).toBe(f.source);
  expect(connections[0]?.target).toBe(f.target);
  restoreNativeCubeConnections(f.graph, connections);
  expect(f.connect).not.toHaveBeenCalled();
  f.disconnect();
  restoreNativeCubeConnections(f.graph, connections);
  expect(f.connect).toHaveBeenCalledWith(1, f.target, 0);
  expect(f.target.inputs[0]?.link).toBe('restored-link');
  restoreNativeCubeConnections(f.graph, connections);
  expect(f.connect).toHaveBeenCalledTimes(1);
});

test('reports a native connect that declines restoration instead of silently losing an edge', () => {
  const f = fixture();
  const connections = captureNativeCubeConnections(f.graph, [f.target]);
  f.disconnect();
  f.connect.mockImplementation(() => undefined);
  expect(() => restoreNativeCubeConnections(f.graph, connections)).toThrow('could not be restored');
});

test.each(['link', 'endpoint', 'slot'] as const)(
  'rejects an unavailable %s before native mutation',
  (kind) => {
    const f = fixture();
    if (kind === 'link') f.links.clear();
    if (kind === 'endpoint') f.nodes.delete('source');
    if (kind === 'slot')
      f.links.set('original-link', {
        origin_id: 'source',
        target_id: 'target',
        origin_slot: NaN,
        target_slot: 0,
      });
    expect(() => captureNativeCubeConnections(f.graph, [f.target])).toThrow('Cube connection');
    expect(f.connect).not.toHaveBeenCalled();
  },
);

/** Supply native connection mutation and normalized graph lookup without replacing endpoint objects. */
function fixture() {
  const source = node('source');
  const target = node('target');
  source.outputs.push({ name: 'other', links: [] }, { name: 'image', links: ['original-link'] });
  target.inputs.push({ name: 'image', link: 'original-link' });
  const nodes = new Map<GraphId, CubeNode>([
    ['source', source],
    ['target', target],
  ]);
  const links = new Map<GraphId, ComfyLink>([
    ['original-link', { origin_id: 'source', origin_slot: 1, target_id: 'target', target_slot: 0 }],
  ]);
  const connect = jest.fn<CubeNode['connect']>((slot, destination, inputSlot) => {
    const input = destination.inputs[inputSlot];
    if (!input) throw new Error('Unavailable test input');
    input.link = 'restored-link';
    links.set('restored-link', {
      origin_id: source.id,
      origin_slot: slot,
      target_id: destination.id,
      target_slot: inputSlot,
    });
  });
  source.connect = connect;
  const graph = {
    getNodeById: (id: GraphId) => nodes.get(id),
    getLink: (id: GraphId) => links.get(id) ?? null,
  };
  return {
    source,
    target,
    nodes,
    links,
    connect,
    graph,
    disconnect: () => {
      links.clear();
      target.inputs[0]!.link = null;
    },
  };
}

/** Create a typed native Cube endpoint with the graph surface used by connection validation. */
function node(id: string): CubeNode {
  return {
    id,
    pos: [0, 0],
    size: [920, 600],
    properties: {},
    inputs: [],
    outputs: [],
    connect() {},
    serialize: () => ({}),
    isSubgraphNode: () => true,
    subgraph: {
      id: `definition-${id}`,
      name: id,
      _nodes: [],
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add() {},
      remove() {},
      configure() {},
      addInput: () => ({ connect() {} }),
      addOutput: () => ({ connect() {} }),
    },
  };
}
