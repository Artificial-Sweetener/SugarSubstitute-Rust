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
/** Prove complete carts use one native transaction and preserve existing objects on failure. */
import { expect, jest, test } from '@jest/globals';
import { ComfyCubeStackCommitHost } from '../../../frontend/comfyui/ui/cube/stack/ComfyCubeStackCommitHost.js';
import type { CubeStackCreation } from '../../../frontend/comfyui/ui/cube/stack/ComfyCubeStackCommitHost.js';
import type { ConstructedCube } from '../../../frontend/comfyui/ui/cube/CubeConstructionService.js';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type {
  CubeGraphAnalysis,
  CubeGraphInstance,
} from '../../../frontend/comfyui/ui/workflow/domain/CubeGraphModels.js';
import type { GraphId } from '../../../frontend/comfyui/ui/types/graph.js';

test('adds, removes and moves with one coherent publication and a balanced history boundary', () => {
  const f = fixture();
  const vector = f.first.pos;
  const widgets = f.first.widgets;
  f.host.commit(f.snapshot, plan(['added', 'first']), [f.creation]);
  expect([...f.root.values()]).toEqual([f.first, f.added]);
  expect(f.catalog.list()).toEqual([f.first, f.added]);
  expect(f.first.pos).toBe(vector);
  expect(f.first.widgets).toBe(widgets);
  expect([...f.first.pos]).toEqual([244, 0]);
  expect(f.construct).toHaveBeenCalledWith(
    f.creation.payload,
    expect.objectContaining({ instanceId: 'added', instanceAlias: 'added', size: [920, 600] }),
  );
  expect(f.publications).toEqual([['first', 'added']]);
  expect(f.before).toHaveBeenCalledTimes(1);
  expect(f.after).toHaveBeenCalledTimes(1);
  expect(f.discard).not.toHaveBeenCalled();
});

test.each(['construction', 'insertion', 'removal', 'position'] as const)(
  'compensates %s failure without replacing existing nodes or vectors',
  (failure) => {
    const f = fixture();
    const firstVector = f.first.pos;
    if (failure === 'construction')
      f.construct.mockImplementation(() => {
        throw new Error('construction failure');
      });
    if (failure === 'insertion') f.failAfterAdd();
    if (failure === 'removal') f.failAfterRemove();
    if (failure === 'position')
      f.first.setPos = (x, y) => {
        f.first.pos[0] = x;
        f.first.pos[1] = y;
        throw new Error('position failure');
      };
    expect(() => f.host.commit(f.snapshot, plan(['added', 'first']), [f.creation])).toThrow(
      `${failure} failure`,
    );
    expect(new Set(f.root.values())).toEqual(new Set([f.first, f.second]));
    expect(new Set(f.catalog.list())).toEqual(new Set([f.first, f.second]));
    expect(f.first.pos).toBe(firstVector);
    expect([...f.first.pos]).toEqual([0, 0]);
    expect([...f.second.pos]).toEqual([244, 0]);
    expect(f.before).toHaveBeenCalledTimes(1);
    expect(f.after).toHaveBeenCalledTimes(1);
    expect(f.discardDefinitions).toHaveBeenCalledTimes(1);
    expect(f.publications).toHaveLength(
      failure === 'construction' || failure === 'insertion' ? 0 : 1,
    );
  },
);

test('declines stale, incomplete and duplicate plans before native history begins', () => {
  const f = fixture();
  const target = plan(['first', 'second']);
  expect(() => f.host.commit({ ...f.snapshot, revision: 'stale' }, target, [])).toThrow(
    'Workflow changed',
  );
  expect(() =>
    f.host.commit(
      f.snapshot,
      { ...target, placements: [...target.placements, target.placements[0]!] },
      [],
    ),
  ).toThrow('duplicate');
  expect(() => f.host.commit(f.snapshot, plan(['added', 'first']), [])).toThrow('unprepared');
  f.retire();
  expect(() => f.host.commit(f.snapshot, target, [])).toThrow('Workflow changed');
  expect(f.before).not.toHaveBeenCalled();
});

test('unchanged carts produce no history transaction or publication', () => {
  const f = fixture();
  f.host.commit(f.snapshot, plan(['first', 'second']), []);
  expect(f.before).not.toHaveBeenCalled();
  expect(f.publications).toEqual([]);
});

test('reports both construction and failed definition cleanup', () => {
  const f = fixture();
  const original = new Error('construction failure');
  const cleanup = new Error('cleanup failure');
  f.construct.mockImplementation(() => {
    throw original;
  });
  f.discardDefinitions.mockImplementation(() => {
    throw cleanup;
  });
  try {
    f.host.commit(f.snapshot, plan(['first', 'added']), [f.creation]);
    throw new Error('Expected construction failure');
  } catch (error: unknown) {
    expect(error).toBeInstanceOf(AggregateError);
    if (error instanceof AggregateError) expect(error.errors).toEqual([original, cleanup]);
  }
  expect(f.after).toHaveBeenCalledTimes(1);
});

/** Supply real catalog and mutable native vectors with failure injection only at host APIs. */
function fixture() {
  const first = node('first', 0);
  const second = node('second', 244);
  const added = node('added', 0);
  const root = new Map<GraphId, CubeNode>([
    [first.id, first],
    [second.id, second],
  ]);
  const catalog = new CubeNodeCatalog();
  catalog.add(first);
  catalog.add(second);
  const publications: string[][] = [];
  catalog.subscribe(() => publications.push(catalog.list().map((value) => String(value.id))));
  const snapshot = { revision: 'opening', serializedWorkflow: '{}' };
  let available = true;
  let addFailure = false;
  let removeFailure = false;
  const before = jest.fn();
  const after = jest.fn();
  const construct = jest.fn(
    (): ConstructedCube => ({
      node: added,
      subgraph: added.subgraph,
      warnings: [],
      internalNodeCount: 0,
    }),
  );
  const discard = jest.fn();
  const discardDefinitions = jest.fn();
  const creation: CubeStackCreation = {
    instanceId: 'added',
    payload: { cube: { cube_id: 'demo' } },
    size: [920, 600],
  };
  const host = new ComfyCubeStackCommitHost({
    snapshots: {
      get available() {
        return available;
      },
      snapshot: () => snapshot,
    },
    graph: {
      add(value) {
        root.set(value.id, value);
        if (addFailure) {
          addFailure = false;
          throw new Error('insertion failure');
        }
      },
      remove(value) {
        root.delete(value.id);
        if (removeFailure) {
          removeFailure = false;
          throw new Error('removal failure');
        }
      },
      getNodeById: (id) => root.get(id),
      getLink: () => null,
    },
    nodes: catalog,
    history: { beforeChange: before, afterChange: after, setDirtyCanvas: jest.fn() },
    construction: { construct, discard },
    definitions: {
      stage: (payload) => ({ payload, definitionIds: ['nested'] }),
      discard: discardDefinitions,
    },
  });
  return {
    host,
    first,
    second,
    added,
    root,
    catalog,
    snapshot,
    creation,
    construct,
    discard,
    discardDefinitions,
    publications,
    before,
    after,
    retire: () => {
      available = false;
    },
    failAfterAdd: () => {
      addFailure = true;
    },
    failAfterRemove: () => {
      removeFailure = true;
    },
  };
}

/** Construct a native subgraph-node fixture with stable live widgets and vector identity. */
function node(id: string, x: number): CubeNode {
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
    pos: new Float32Array([x, 0]),
    size: [920, 600],
    properties: { sugarcubes_cube: { instance_id: id } },
    inputs: [],
    outputs: [],
    widgets: [{ name: 'prompt', value: 'retained' }],
    subgraph,
    isSubgraphNode: () => true,
    serialize: () => ({}),
    connect() {},
  };
}

/** Describe the backend-approved final order without using native numeric identifiers for new instances. */
function plan(ids: readonly string[]): CubeGraphAnalysis {
  const instances: CubeGraphInstance[] = ids.map((id) => ({
    instanceId: id,
    nodeId: id === 'added' ? 'server-node-id' : id,
    definitionId: `definition-${id}`,
    cubeId: 'demo',
    version: '1',
    alias: id,
    executionMode: 0,
  }));
  return {
    semanticHash: 'hash',
    instances,
    segments: [{ instanceIds: ids, reorderable: true, boundaryNodeIds: [] }],
    placements: instances.map((instance, index) => ({
      instanceId: instance.instanceId,
      nodeId: instance.nodeId,
      position: [index * 244, 0],
    })),
  };
}
