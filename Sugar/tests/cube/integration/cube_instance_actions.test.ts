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
/** Prove direct instance actions preserve definitions and commit one undoable native change. */
import { expect, jest, test } from '@jest/globals';
import { CubeInstanceActions } from '../../../frontend/comfyui/ui/cube/stack/CubeInstanceActions.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import {
  requireCubeIdentity,
  type CubeNode,
} from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { GraphId } from '../../../frontend/comfyui/ui/types/graph.js';

test('renames literal instance text while retaining shared definition identity', () => {
  const f = fixture();
  f.actions.rename('source', '<img src=x>');
  expect(f.source.title).toBe('<img src=x>');
  expect(f.source.properties.sugarcubes_cube).toMatchObject({
    instance_alias: '<img src=x>',
    cube_id: 'demo.cube',
  });
  expect(f.source.subgraph.name).toBe('Shared definition');
  expect(f.before).toHaveBeenCalledTimes(1);
  expect(f.after).toHaveBeenCalledTimes(1);
  expect(() => f.actions.rename('source', ' ')).toThrow('name is required');
  expect(f.before).toHaveBeenCalledTimes(1);
});

test('toggles bypass independently of internal node modes and restores muted state', () => {
  const f = fixture();
  f.source.mode = 2;
  f.actions.toggleBypass('source');
  expect(f.source.mode).toBe(4);
  f.actions.toggleBypass('source');
  expect(f.source.mode).toBe(2);
  expect(f.source.properties).not.toHaveProperty('sugarcubes_before_bypass_mode');
  expect(f.before).toHaveBeenCalledTimes(2);
  expect(f.after).toHaveBeenCalledTimes(2);
});

test('persists output saving independently of bypass and shared Cube definition', () => {
  const f = fixture();
  f.actions.toggleOutputPersistence('source');
  expect(f.source.properties.sugarcubes_save_outputs).toBe(false);
  expect(f.source.mode).toBe(0);
  f.actions.toggleOutputPersistence('source');
  expect(f.source.properties.sugarcubes_save_outputs).toBe(true);
});

test('duplicates live native values with fresh identity, copy label and no source mutation', () => {
  const f = fixture();
  f.source.properties.sugarcubes_save_outputs = false;
  const copy = f.actions.duplicate('source');
  expect(copy.id).toBe('copy');
  expect(copy.title).toBe('Demo 2');
  expect(copy.properties.sugarcubes_cube).toMatchObject({
    instance_id: 'copy',
    instance_alias: 'Demo 2',
    cube_id: 'demo.cube',
  });
  expect(copy.properties.sugarcubes_save_outputs).toBe(false);
  expect(copy.widgets).toEqual(f.source.widgets);
  expect(copy.widgets).not.toBe(f.source.widgets);
  expect(copy.pos).toEqual([224, 0]);
  expect(f.source.title).toBe('Demo');
  expect(f.source.properties.sugarcubes_cube).toMatchObject({ instance_id: 'source' });
  expect(f.catalog.get('copy')).toBe(copy);
  expect(f.before).toHaveBeenCalledTimes(1);
  expect(f.after).toHaveBeenCalledTimes(1);
});

test('removes the selected Cube directly in one coherent native transaction', () => {
  const f = fixture();
  f.actions.remove('source');
  expect(f.root.size).toBe(0);
  expect(f.catalog.list()).toEqual([]);
  expect(f.before).toHaveBeenCalledTimes(1);
  expect(f.after).toHaveBeenCalledTimes(1);
});

test.each(['insertion', 'removal'] as const)(
  'compensates a partial native %s failure',
  (failure) => {
    const f = fixture();
    f.fail(failure);
    expect(() =>
      failure === 'insertion' ? f.actions.duplicate('source') : f.actions.remove('source'),
    ).toThrow(failure);
    expect([...f.root.values()]).toEqual([f.source]);
    expect(f.catalog.list()).toEqual([f.source]);
    expect(f.before).toHaveBeenCalledTimes(1);
    expect(f.after).toHaveBeenCalledTimes(1);
  },
);

test('rejects retired workflow actions before invoking history or changing nodes', () => {
  const f = fixture();
  f.retire();
  expect(() => f.actions.remove('source')).toThrow('retired');
  expect(f.before).not.toHaveBeenCalled();
  expect(f.catalog.get('source')).toBe(f.source);
});

/** Supply native lifecycle boundaries with injected partial host failures. */
function fixture() {
  const source = cube();
  const catalog = new CubeNodeCatalog();
  catalog.add(source);
  const root = new Map<GraphId, CubeNode>([[source.id, source]]);
  const before = jest.fn();
  const after = jest.fn();
  let retired = false;
  let failure = '';
  const actions = new CubeInstanceActions({
    nodes: catalog,
    history: { beforeChange: before, afterChange: after, setDirtyCanvas: jest.fn() },
    scope: {
      assertCurrentRoot: () => {
        if (retired) throw new Error('retired workflow');
      },
    },
    isReordering: () => false,
    createId: () => 'copy',
    graph: {
      getNodeById: (id) => root.get(id),
      getLink: () => null,
      add: (node) => {
        root.set(node.id, node);
        if (failure === 'insertion') {
          failure = '';
          throw new Error('insertion');
        }
      },
      remove: (node) => {
        root.delete(node.id);
        if (failure === 'removal') {
          failure = '';
          throw new Error('removal');
        }
      },
    },
  });
  return {
    source,
    actions,
    root,
    catalog,
    before,
    after,
    retire: () => {
      retired = true;
    },
    fail: (value: string) => {
      failure = value;
    },
  };
}

/** Model the official native clone contract with detached links and independent serialized values. */
function cube(): CubeNode {
  const subgraph: NativeCubeSubgraph = {
    id: 'definition',
    name: 'Shared definition',
    _nodes: [],
    inputs: [],
    outputs: [],
    inputNode: {},
    outputNode: {},
    add() {},
    remove() {},
    configure() {},
    addInput() {
      return { connect() {} };
    },
    addOutput() {
      return { connect() {} };
    },
  };
  const node: CubeNode = {
    id: 'source',
    title: 'Demo',
    mode: 0,
    pos: [0, 0],
    size: [200, 300],
    inputs: [],
    outputs: [],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'source', instance_alias: 'Demo', cube_id: 'demo.cube' },
    },
    widgets: [{ name: 'prompt', value: 'live edited prompt' }],
    subgraph,
    isSubgraphNode: () => true,
    serialize: () => ({}),
    connect() {},
  };
  node.clone = () => ({
    ...node,
    pos: [...node.pos],
    size: [...node.size],
    inputs: [],
    outputs: [],
    properties: { ...node.properties, sugarcubes_cube: { ...requireCubeIdentity(node) } },
    widgets: node.widgets?.map((widget) => ({ ...widget })),
  });
  return node;
}
