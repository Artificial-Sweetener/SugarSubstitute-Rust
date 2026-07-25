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
/** Characterize Cube identity and catalog reconciliation across Comfy graph changes. */

import { jest } from '@jest/globals';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { ComfyCubeNodeLifecycleAdapter } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeLifecycleAdapter.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

describe('ComfyCubeNodeLifecycleAdapter', () => {
  test('reidentifies a copied Cube after Comfy finishes configuring the paste', () => {
    const original = cubeNode('original-instance');
    const pasted = cubeNode('original-instance');
    const graph = { _nodes: [original] as unknown[] };
    const events = new EventTarget();
    const catalog = new CubeNodeCatalog();
    const createInstanceId = jest.fn(() => 'pasted-instance');
    const adapter = new ComfyCubeNodeLifecycleAdapter({
      graph,
      catalog,
      events,
      createInstanceId,
      logger: { debug: jest.fn(), error: jest.fn() },
    });

    graph._nodes.push(pasted);
    events.dispatchEvent(graphChangeEvent());

    expect(createInstanceId).toHaveBeenCalledTimes(1);
    expect(pasted.properties.sugarcubes_cube).toMatchObject({
      instance_id: 'pasted-instance',
    });
    expect(catalog.list()).toEqual([original, pasted]);
    adapter.dispose();
  });

  test('preserves a uniquely identified Cube restored by undo', () => {
    const restored = cubeNode('restored-instance');
    const graph = {
      _nodes: [] as unknown[],
      onNodeAdded: undefined as ((node: unknown) => void) | undefined,
    };
    const events = new EventTarget();
    const catalog = new CubeNodeCatalog();
    const createInstanceId = jest.fn(() => 'unused-instance');
    const adapter = new ComfyCubeNodeLifecycleAdapter({
      graph,
      catalog,
      events,
      createInstanceId,
      logger: { debug: jest.fn(), error: jest.fn() },
    });

    graph._nodes.push(restored);
    graph.onNodeAdded?.(restored);
    events.dispatchEvent(graphChangeEvent());

    expect(createInstanceId).not.toHaveBeenCalled();
    expect(catalog.get('restored-instance')).toBe(restored);
    adapter.dispose();
  });

  test('restores graph hooks and stops event reconciliation on disposal', () => {
    const previousAdded = jest.fn();
    const graph = {
      _nodes: [] as unknown[],
      onNodeAdded: previousAdded,
    };
    const events = new EventTarget();
    const catalog = new CubeNodeCatalog();
    const adapter = new ComfyCubeNodeLifecycleAdapter({
      graph,
      catalog,
      events,
      createInstanceId: () => 'unused-instance',
      logger: { debug: jest.fn(), error: jest.fn() },
    });
    const installedAdded = graph.onNodeAdded;

    adapter.dispose();
    graph._nodes.push(cubeNode('after-dispose'));
    events.dispatchEvent(graphChangeEvent());

    expect(installedAdded).not.toBe(previousAdded);
    expect(graph.onNodeAdded).toBe(previousAdded);
    expect(catalog.list()).toEqual([]);
  });
});

/** Build the public canvas event Comfy emits after paste configuration. */
function graphChangeEvent(): CustomEvent<{ subType: string }> {
  return new CustomEvent('litegraph:canvas', {
    detail: { subType: 'after-change' },
  });
}

/** Build a real-subgraph-node double with durable Cube metadata. */
function cubeNode(instanceId: string): CubeNode {
  const subgraph = {
    id: `definition-${instanceId}`,
    name: 'Cube',
    _nodes: [],
    inputs: [],
    outputs: [],
  } as unknown as NativeCubeSubgraph;
  return {
    id: instanceId,
    type: subgraph.id,
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: instanceId },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
