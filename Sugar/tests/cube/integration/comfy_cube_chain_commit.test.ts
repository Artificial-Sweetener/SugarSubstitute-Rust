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
/** Prove a chain change is one undoable step that either fully applies or leaves the graph as it was. */
import { expect, jest, test } from '@jest/globals';
import {
  ComfyCubeChainCommitHost,
  type CubeChainChange,
  type CubeChainCreation,
} from '../../../frontend/comfyui/ui/cube/chain/ComfyCubeChainCommitHost.js';
import type { ConstructedCube } from '../../../frontend/comfyui/ui/cube/CubeConstructionService.js';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { ComfyLink, GraphId } from '../../../frontend/comfyui/ui/types/graph.js';

test('adds, removes and moves in one step without replacing kept nodes or vectors', () => {
  const f = fixture();
  const vector = f.first.pos;
  const widgets = f.first.widgets;
  f.host.commit(
    change({
      creations: [f.creation],
      removals: ['second'],
      at: { added: [0, 0], first: [944, 0] },
    }),
  );
  expect([...f.root.values()]).toEqual([f.first, f.added]);
  expect(f.catalog.list()).toEqual([f.first, f.added]);
  expect(f.first.pos).toBe(vector);
  expect(f.first.widgets).toBe(widgets);
  expect([...f.first.pos]).toEqual([944, 0]);
  expect(f.construct).toHaveBeenCalledWith(
    f.creation.payload,
    expect.objectContaining({ instanceId: 'added', instanceAlias: 'Added', size: [920, 600] }),
  );
  expect(f.reconcile).toHaveBeenCalledTimes(1);
  expect(f.before).toHaveBeenCalledTimes(1);
  expect(f.after).toHaveBeenCalledTimes(1);
  expect(f.discard).not.toHaveBeenCalled();
});

test('rewires a hardwired Cube in the same step', () => {
  const f = fixture();
  f.link('first', 0, 'second', 0);
  f.host.commit(
    change({
      at: { second: [0, 0], first: [944, 0] },
      links: {
        remove: [
          { sourceInstance: 'first', sourceSlot: 0, targetInstance: 'second', targetSlot: 0 },
        ],
        add: [{ sourceInstance: 'second', sourceSlot: 0, targetInstance: 'first', targetSlot: 0 }],
      },
    }),
  );
  expect(f.second.inputs[0]?.link).toBeNull();
  expect(f.links.get(f.first.inputs[0]?.link ?? -1)).toMatchObject({
    origin_id: 'second',
    target_id: 'first',
  });
  expect(f.before).toHaveBeenCalledTimes(1);
});

test.each(['construction', 'insertion', 'removal', 'position', 'wiring'] as const)(
  'undoes everything after a %s failure, keeping existing nodes and vectors',
  (failure) => {
    const f = fixture();
    f.link('first', 0, 'second', 0);
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
    if (failure === 'wiring') f.failConnect();
    expect(() =>
      f.host.commit(
        change({
          creations: [f.creation],
          removals: failure === 'removal' ? ['second'] : [],
          at: { added: [0, 0], first: [944, 0] },
          links: {
            remove: [
              { sourceInstance: 'first', sourceSlot: 0, targetInstance: 'second', targetSlot: 0 },
            ],
            add:
              failure === 'removal'
                ? []
                : [
                    {
                      sourceInstance: 'added',
                      sourceSlot: 0,
                      targetInstance: 'second',
                      targetSlot: 0,
                    },
                  ],
          },
        }),
      ),
    ).toThrow(failure === 'wiring' ? 'could not be wired' : `${failure} failure`);
    expect(new Set(f.root.values())).toEqual(new Set([f.first, f.second]));
    expect(new Set(f.catalog.list())).toEqual(new Set([f.first, f.second]));
    expect(f.first.pos).toBe(firstVector);
    expect([...f.first.pos]).toEqual([0, 0]);
    expect([...f.second.pos]).toEqual([944, 0]);
    expect(f.links.get(f.second.inputs[0]?.link ?? -1)).toMatchObject({ origin_id: 'first' });
    expect(f.before).toHaveBeenCalledTimes(1);
    expect(f.after).toHaveBeenCalledTimes(1);
    expect(f.discardDefinitions).toHaveBeenCalledTimes(1);
  },
);

test('an unchanged chain opens no undo step', () => {
  const f = fixture();
  f.host.commit(change({ at: { first: [0, 0], second: [944, 0] } }));
  expect(f.before).not.toHaveBeenCalled();
  expect(f.reconcile).not.toHaveBeenCalled();
});

test('reports both a construction failure and a failed definition cleanup', () => {
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
    f.host.commit(change({ creations: [f.creation], at: { added: [0, 0] } }));
    throw new Error('Expected construction failure');
  } catch (error: unknown) {
    expect(error).toBeInstanceOf(AggregateError);
    if (error instanceof AggregateError) expect(error.errors[0]).toBe(original);
  }
  expect(f.after).toHaveBeenCalledTimes(1);
});

/** Describe one chain change by instance id. */
function change(parts: {
  creations?: CubeChainCreation[];
  removals?: string[];
  at?: Record<string, [number, number]>;
  links?: CubeChainChange['links'];
}): CubeChainChange {
  return {
    creations: parts.creations ?? [],
    additions: [],
    removals: parts.removals ?? [],
    positions: new Map(Object.entries(parts.at ?? {})),
    links: parts.links ?? { remove: [], add: [] },
  };
}

/** Supply a real catalog and mutable native vectors with failures injected only at host APIs. */
function fixture() {
  const links = new Map<GraphId, ComfyLink>();
  let nextLink = 1;
  let connectFailure = false;
  const first = node('first', 0);
  const second = node('second', 944);
  const added = node('added', 0);
  const root = new Map<GraphId, CubeNode>([
    [first.id, first],
    [second.id, second],
  ]);
  for (const value of [first, second, added]) {
    value.connect = (sourceSlot: number, target: unknown, targetSlot: number) => {
      if (connectFailure) {
        connectFailure = false;
        return null;
      }
      const to = target as CubeNode;
      const id = nextLink++;
      links.set(id, {
        id,
        origin_id: value.id,
        origin_slot: sourceSlot,
        target_id: to.id,
        target_slot: targetSlot,
      });
      const input = to.inputs[targetSlot];
      if (input) input.link = id;
      return {};
    };
    Object.assign(value, {
      disconnectInput: (slot: number) => {
        const input = value.inputs[slot];
        if (input?.link !== null && input?.link !== undefined) links.delete(input.link);
        if (input) input.link = null;
      },
    });
  }
  const catalog = new CubeNodeCatalog();
  catalog.add(first);
  catalog.add(second);
  let addFailure = false;
  let removeFailure = false;
  const before = jest.fn();
  const after = jest.fn();
  const reconcile = jest.fn();
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
  const creation: CubeChainCreation = {
    instanceId: 'added',
    alias: 'Added',
    payload: { cube: { cube_id: 'demo' } },
    size: [920, 600],
  };
  const host = new ComfyCubeChainCommitHost({
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
      getLink: (id) => links.get(id) ?? null,
    },
    nodes: catalog,
    history: { beforeChange: before, afterChange: after, setDirtyCanvas: jest.fn() },
    construction: { construct, discard },
    definitions: {
      stage: (payload) => ({ payload, definitionIds: ['nested'] }),
      discard: discardDefinitions,
    },
    reconcile,
    announce: () => undefined,
  });
  return {
    host,
    first,
    second,
    added,
    root,
    links,
    catalog,
    creation,
    construct,
    discard,
    discardDefinitions,
    reconcile,
    before,
    after,
    link: (source: string, sourceSlot: number, target: string, targetSlot: number) => {
      const from = root.get(source);
      const to = root.get(target);
      if (from && to) from.connect(sourceSlot, to, targetSlot);
    },
    failAfterAdd: () => {
      addFailure = true;
    },
    failAfterRemove: () => {
      removeFailure = true;
    },
    failConnect: () => {
      connectFailure = true;
    },
  };
}

/** Construct a native subgraph-node fixture with one image port each way. */
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
    title: id,
    pos: new Float32Array([x, 0]),
    size: [920, 600],
    properties: { sugarcubes_cube: { instance_id: id } },
    inputs: [{ name: 'image', type: 'IMAGE', link: null }],
    outputs: [{ name: 'image', type: 'IMAGE', links: [] }],
    widgets: [{ name: 'prompt', value: 'retained' }],
    subgraph,
    isSubgraphNode: () => true,
    serialize: () => ({}),
    connect() {},
  };
}
