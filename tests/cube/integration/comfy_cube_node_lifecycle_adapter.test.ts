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
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { ComfyCubeNodeLifecycleAdapter } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeLifecycleAdapter.js';
import { CubeGraphInventory } from '../../../frontend/comfyui/ui/cube/node/CubeGraphInventory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeWorkflowNodePackMetadata } from '../../../frontend/comfyui/ui/cube/node/CubeWorkflowNodePackMetadata.js';

describe('ComfyCubeNodeLifecycleAdapter', () => {
  test('isolates a copied Cube definition before relation discovery can share mutable widget owners', () => {
    const original = cubeNode('first');
    const copied = cubeNode('second');
    copied.subgraph = original.subgraph;
    copied.type = original.type;
    original.subgraph.asSerialisable = () => ({
      id: original.subgraph.id,
      name: 'Cube',
      nodes: [],
    });
    original.subgraph.inputNode = {};
    original.subgraph.outputNode = {};
    original.subgraph.add = () => undefined;
    original.subgraph.remove = () => undefined;
    original.subgraph.addInput = () => ({ connect() {} });
    original.subgraph.addOutput = () => ({ connect() {} });
    original.subgraph.configure = () => undefined;
    let nextId = 0;
    const definitions = new Map([[original.subgraph.id, original.subgraph]]);
    const graph = withTopology({
      _nodes: [original, copied] as unknown[],
      subgraphs: definitions,
      createSubgraph: (data: { id: string }) => {
        const definition = cubeNode(`isolated-${++nextId}`).subgraph;
        definition.id = data.id;
        definition.inputNode = {};
        definition.outputNode = {};
        definition.add = () => undefined;
        definition.remove = () => undefined;
        definition.addInput = () => ({ connect() {} });
        definition.addOutput = () => ({ connect() {} });
        definition.configure = () => undefined;
        definitions.set(data.id, definition);
        return definition;
      },
    });
    const catalog = new CubeNodeCatalog();
    const adapter = new ComfyCubeNodeLifecycleAdapter({
      discardDefinition: jest.fn(),
      graph,
      inventory: new CubeGraphInventory(graph),
      catalog,
      events: new EventTarget(),
      createInstanceId: () => 'unused',
      nodePackMetadata: unconfiguredNodePackMetadata(),
      logger: { debug: jest.fn(), error: jest.fn() },
    });
    expect(copied.subgraph).not.toBe(original.subgraph);
    expect(copied.type).toBe(copied.subgraph.id);
    expect(copied.properties.sugarcubes_cube).toMatchObject({ instance_id: 'second' });
    expect(catalog.list()).toEqual([original, copied]);
    adapter.dispose();
  });

  test('reconciles legacy root Cube dependency metadata without shifting workflow state', () => {
    const cube = cubeNode('legacy-instance');
    cube.properties.persisted_value = 2;
    const nested = cubeNode('nested-instance');
    const graph = {
      _nodes: [cube] as unknown[],
      subgraphs: new Map([['nested-definition', { _nodes: [nested] as unknown[] }]]),
    };
    const nodePackMetadata = new CubeWorkflowNodePackMetadata();
    const adapter = new ComfyCubeNodeLifecycleAdapter({
      discardDefinition: jest.fn(),
      graph: withTopology(graph),
      inventory: new CubeGraphInventory(graph),
      catalog: new CubeNodeCatalog(),
      events: new EventTarget(),
      createInstanceId: () => 'unused-instance',
      nodePackMetadata,
      logger: { debug: jest.fn(), error: jest.fn() },
    });

    nodePackMetadata.updateFromStatus({
      workflowNodePack: { cnrId: 'SugarCubes', version: '9.8.7' },
    });

    expect(cube.properties).toMatchObject({
      cnr_id: 'SugarCubes',
      ver: '9.8.7',
      persisted_value: 2,
      sugarcubes_cube: { instance_id: 'legacy-instance' },
    });
    expect(cube.id).toBe('legacy-instance');
    expect(cube.type).toBe('definition-legacy-instance');
    expect(nested.properties.cnr_id).toBeUndefined();
    expect(nested.properties.ver).toBeUndefined();
    adapter.dispose();
  });

  test('does not wake presentation when authoritative graph membership is unchanged', () => {
    const first = cubeNode('first-instance');
    const second = cubeNode('second-instance');
    const catalog = new CubeNodeCatalog();
    const changed = jest.fn();
    catalog.subscribe(changed);

    catalog.replace([first, second]);
    catalog.replace([first, second]);

    expect(changed).toHaveBeenCalledTimes(1);
    catalog.replace([second, first]);
    expect(changed).toHaveBeenCalledTimes(2);
  });

  test('reidentifies a copied Cube after Comfy finishes configuring the paste', () => {
    const original = cubeNode('original-instance');
    const pasted = cubeNode('original-instance');
    const graph = { _nodes: [original] as unknown[] };
    const events = new EventTarget();
    const catalog = new CubeNodeCatalog();
    const createInstanceId = jest.fn(() => 'pasted-instance');
    const adapter = new ComfyCubeNodeLifecycleAdapter({
      discardDefinition: jest.fn(),
      graph: withTopology(graph),
      inventory: new CubeGraphInventory(graph),
      catalog,
      events,
      createInstanceId,
      nodePackMetadata: unconfiguredNodePackMetadata(),
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
      discardDefinition: jest.fn(),
      graph: withTopology(graph),
      inventory: new CubeGraphInventory(graph),
      catalog,
      events,
      createInstanceId,
      nodePackMetadata: unconfiguredNodePackMetadata(),
      logger: { debug: jest.fn(), error: jest.fn() },
    });

    graph._nodes.push(restored);
    graph.onNodeAdded?.(restored);
    events.dispatchEvent(graphChangeEvent());

    expect(createInstanceId).not.toHaveBeenCalled();
    expect(catalog.get('restored-instance')).toBe(restored);
    adapter.dispose();
  });

  test('keeps the active renderer reachable across runtime replacement and repeated undo/redo', () => {
    const rendered = new Set<unknown>();
    const graph = {
      _nodes: [] as unknown[],
      onNodeAdded: (node: unknown): void => {
        rendered.add(node);
      },
      onNodeRemoved: (node: unknown): void => {
        rendered.delete(node);
      },
    };
    const nativeAdded = graph.onNodeAdded;
    const nativeRemoved = graph.onNodeRemoved;
    const events = new EventTarget();
    for (let restore = 0; restore < 6; restore += 1) {
      const catalog = new CubeNodeCatalog();
      const adapter = new ComfyCubeNodeLifecycleAdapter({
        discardDefinition: jest.fn(),
        graph: withTopology(graph),
        inventory: new CubeGraphInventory(graph),
        catalog,
        events,
        createInstanceId: () => 'unused-instance',
        nodePackMetadata: unconfiguredNodePackMetadata(),
        logger: { debug: jest.fn(), error: jest.fn() },
      });
      expect(graph.onNodeAdded).toBe(nativeAdded);
      expect(graph.onNodeRemoved).toBe(nativeRemoved);
      for (const node of graph._nodes) graph.onNodeRemoved(node);
      const restored = cubeNode('restored-instance');
      graph._nodes = [restored];
      graph.onNodeAdded(restored);
      events.dispatchEvent(graphChangeEvent());
      expect(rendered).toEqual(new Set([restored]));
      expect(catalog.list()).toEqual([restored]);
      adapter.dispose();
    }
  });

  test('reconciles programmatic addition and removal after native topology completes', async () => {
    const original = cubeNode('original-instance');
    const graph = withTopology({ _nodes: [original] as unknown[] });
    const catalog = new CubeNodeCatalog();
    const adapter = new ComfyCubeNodeLifecycleAdapter({
      discardDefinition: jest.fn(),
      graph,
      inventory: new CubeGraphInventory(graph),
      catalog,
      events: new EventTarget(),
      createInstanceId: () => 'unused-instance',
      nodePackMetadata: unconfiguredNodePackMetadata(),
      logger: { debug: jest.fn(), error: jest.fn() },
    });
    const added = cubeNode('added-instance');
    graph.add(added);
    await Promise.resolve();
    expect(catalog.list()).toEqual([original, added]);
    graph.remove(original);
    await Promise.resolve();
    expect(catalog.list()).toEqual([added]);
    graph.add(cubeNode('disposed-instance'));
    adapter.dispose();
    await Promise.resolve();
    expect(catalog.list()).toEqual([added]);
  });

  test('reconciles configured replacements and ignores obsolete hooks retained by the host', () => {
    const configured = jest.fn();
    const graph = withTopology({ _nodes: [] as unknown[], onConfigure: configured });
    const catalog = new CubeNodeCatalog();
    const adapter = new ComfyCubeNodeLifecycleAdapter({
      discardDefinition: jest.fn(),
      graph,
      inventory: new CubeGraphInventory(graph),
      catalog,
      events: new EventTarget(),
      createInstanceId: () => 'unused-instance',
      nodePackMetadata: unconfiguredNodePackMetadata(),
      logger: { debug: jest.fn(), error: jest.fn() },
    });
    const oldHook = graph.onConfigure;
    const restored = cubeNode('restored-instance');
    graph._nodes = [restored];
    graph.onConfigure({});
    expect(catalog.list()).toEqual([restored]);
    adapter.dispose();
    graph._nodes = [];
    oldHook({});
    expect(catalog.list()).toEqual([restored]);
    expect(configured).toHaveBeenCalledTimes(2);
  });

  test('excludes invalid nested Cubes from identity normalization and the root catalog', () => {
    const rootCube = cubeNode('shared-instance');
    const nestedCube = cubeNode('shared-instance');
    const graph = {
      _nodes: [rootCube] as unknown[],
      subgraphs: new Map([['editor-definition', { _nodes: [nestedCube] as unknown[] }]]),
    };
    const events = new EventTarget();
    const catalog = new CubeNodeCatalog();
    const adapter = new ComfyCubeNodeLifecycleAdapter({
      discardDefinition: jest.fn(),
      graph: withTopology(graph),
      inventory: new CubeGraphInventory(graph),
      catalog,
      events,
      createInstanceId: () => 'nested-instance',
      nodePackMetadata: unconfiguredNodePackMetadata(),
      logger: { debug: jest.fn(), error: jest.fn() },
    });

    expect(nestedCube.properties.sugarcubes_cube).toMatchObject({
      instance_id: 'shared-instance',
    });
    expect(catalog.list()).toEqual([rootCube]);
    adapter.dispose();
  });

  test('leaves renderer callbacks untouched and stops event reconciliation on disposal', () => {
    const previousAdded = jest.fn();
    const graph = {
      _nodes: [] as unknown[],
      onNodeAdded: previousAdded,
    };
    const events = new EventTarget();
    const catalog = new CubeNodeCatalog();
    const adapter = new ComfyCubeNodeLifecycleAdapter({
      discardDefinition: jest.fn(),
      graph: withTopology(graph),
      inventory: new CubeGraphInventory(graph),
      catalog,
      events,
      createInstanceId: () => 'unused-instance',
      nodePackMetadata: unconfiguredNodePackMetadata(),
      logger: { debug: jest.fn(), error: jest.fn() },
    });
    const installedAdded = graph.onNodeAdded;

    adapter.dispose();
    graph._nodes.push(cubeNode('after-dispose'));
    events.dispatchEvent(graphChangeEvent());

    expect(installedAdded).toBe(previousAdded);
    expect(graph.onNodeAdded).toBe(previousAdded);
    expect(catalog.list()).toEqual([]);
  });
});

/** Build metadata state that cannot mutate a graph until backend identity arrives. */
function unconfiguredNodePackMetadata(): CubeWorkflowNodePackMetadata {
  return new CubeWorkflowNodePackMetadata();
}

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

/** Supply authoritative topology methods without implementing host renderer callbacks. */
function withTopology<T extends { _nodes: unknown[] }>(
  graph: T,
): T & { add(node: unknown): void; remove(node: unknown): void } {
  return Object.assign(graph, {
    add(node: unknown): void {
      graph._nodes.push(node);
    },
    remove(node: unknown): void {
      graph._nodes = graph._nodes.filter((value) => value !== node);
    },
  });
}
