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
/** Prove native and shared seed restore actions preserve ownership and form one history transaction. */
import { jest } from '@jest/globals';
import { NativeRelationWorkflowPort } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationWorkflowPort.js';
import { WorkflowRelations } from '../../../frontend/comfyui/ui/relations/application/WorkflowRelations.js';
import { NativeWorkflowSeedReuse } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeWorkflowSeedReuse.js';
import { WorkflowSeedHistory } from '../../../frontend/comfyui/ui/execution/application/WorkflowSeedHistory.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type {
  ComfyGraph,
  ComfyNode,
  ComfyWidget,
} from '../../../frontend/comfyui/ui/types/graph.js';
import { nativeCube } from '../support/native_cube.js';
import type { NativeGraphNode } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';

/** Expose the same native seed/mode widget relationship that Comfy generation uses. */
function sampler(id: string, name = 'seed'): NativeGraphNode {
  const mode: ComfyWidget = {
    name: 'control_after_generate',
    type: 'combo',
    value: 'randomize',
    serialize: false,
    options: { values: ['fixed', 'randomize', 'increment', 'decrement'] },
  };
  return {
    id,
    pos: [0, 0],
    size: [100, 100],
    properties: {},
    inputs: [],
    outputs: [],
    connect() {},
    type: 'KSampler',
    widgets: [
      {
        name,
        type: 'number',
        value: 99,
        linkedWidgets: [mode],
        options: { min: 0, max: 1000, precision: 0 },
      },
      mode,
    ],
  };
}

/** Bind real native inventory, relation transactions and workflow-qualified accepted seed history. */
function fixture(nodes: ComfyNode[]) {
  const graph: ComfyGraph = {
    _nodes: nodes,
    extra: {},
    beforeChange: jest.fn(),
    afterChange: jest.fn(),
  };
  for (const node of nodes) node.graph = graph;
  const sessions = new WorkflowSessionStore();
  const binding = { sessionId: workflowSessionId('seeds'), title: 'Seeds', path: 'seeds.json' };
  sessions.synchronize([binding], binding.sessionId);
  const context = sessions.read().active;
  if (!context) throw new Error('Missing workflow');
  const history = new WorkflowSeedHistory();
  history.synchronize(sessions.read());
  const port = new NativeRelationWorkflowPort(
    () => graph,
    document,
    console,
    () => null,
  );
  const relations = new WorkflowRelations(port);
  relations.reconcile();
  const error = jest.fn();
  const reuse = new NativeWorkflowSeedReuse(
    { graph },
    sessions,
    history,
    relations,
    port.inventorySource,
    document,
    error,
  );
  return { graph, sessions, history, context, port, relations, error, reuse };
}

test('restores an independent submitted seed and locks it in exactly one native history transaction', () => {
  const node = sampler('one');
  const f = fixture([node]);
  f.relations.setOverrideEnabled('seed', false);
  jest.mocked(f.graph.beforeChange!).mockClear();
  jest.mocked(f.graph.afterChange!).mockClear();
  const control = f.reuse.field(node, 'seed');
  expect(control?.readLast()).toBeNull();
  control?.reuse();
  expect(f.graph.beforeChange).not.toHaveBeenCalled();
  f.history.record(f.context, 1, [{ path: 'one', name: 'seed', value: 42 }]);
  control?.reuse();
  expect(node.widgets?.map((widget) => widget.value)).toEqual([42, 'fixed']);
  expect(f.graph.beforeChange).toHaveBeenCalledTimes(1);
  expect(f.graph.afterChange).toHaveBeenCalledTimes(1);
  expect(f.error).not.toHaveBeenCalled();
});

test('restores a global override from the accepted cohort, updating every field and locking the shared owner', () => {
  const a = sampler('a'),
    b = sampler('b');
  const f = fixture([a, b]);
  f.relations.setSeedMode('seed', 'randomize');
  const control = f.reuse.group('seed');
  f.history.record(f.context, 1, [
    { path: 'a', name: 'seed', value: 42 },
    { path: 'b', name: 'seed', value: 42 },
  ]);
  jest.mocked(f.graph.beforeChange!).mockClear();
  jest.mocked(f.graph.afterChange!).mockClear();
  control.reuse();
  expect(f.relations.snapshot().groups.find((group) => group.key === 'seed')).toMatchObject({
    value: 42,
    seedMode: 'fixed',
  });
  expect(a.widgets?.[0]?.value).toBe(42);
  expect(b.widgets?.[0]?.value).toBe(42);
  expect(f.graph.beforeChange).toHaveBeenCalledTimes(1);
  expect(f.graph.afterChange).toHaveBeenCalledTimes(1);
  expect(f.error).not.toHaveBeenCalled();
});

test('uses the original leaf path for a renamed promoted control and rejects detached owners', () => {
  const leaf = sampler('inner', 'generation_number');
  leaf.inputs = [{ widget: { name: 'generation_number' } }];
  const mode = leaf.widgets?.[1];
  const facade: ComfyNode = {
    id: 'outer',
    widgets: [{ name: 'Exposed seed', type: 'number', value: 99 }],
    subgraph: {
      _nodes: [leaf],
      inputNode: { slots: [{ name: 'Exposed seed', linkIds: [1] }] },
      getLink: () => ({ target_id: 'inner', target_slot: 0 }),
    },
  };
  const f = fixture([facade]);
  const control = f.reuse.field(facade, 'Exposed seed');
  f.history.record(f.context, 1, [{ path: 'outer:inner', name: 'generation_number', value: 0 }]);
  expect(control?.readLast()).toBe(0);
  control?.reuse();
  expect(leaf.widgets?.[0]?.value).toBe(0);
  expect(facade.widgets?.[0]?.value).toBe(0);
  expect(mode?.value).toBe('fixed');
  expect(f.graph.afterChange).toHaveBeenCalledTimes(1);
  f.graph._nodes = [];
  expect(control?.readLast()).toBeNull();
  control?.reuse();
  expect(f.graph.afterChange).toHaveBeenCalledTimes(1);
  expect(f.error).not.toHaveBeenCalled();
});

test('maps native facade and leaf identifiers to the exact compiled cube symbols', () => {
  const leaf = sampler('native-leaf');
  leaf.properties = { sugarcubes_symbol: 'sampler' };
  leaf.inputs = [{ widget: { name: 'seed' } }];
  const face: ComfyNode = {
    id: 'native-face',
    properties: { sugarcubes_symbol: 'sampling' },
    widgets: [{ name: 'seed', type: 'number', value: 99 }],
    subgraph: {
      _nodes: [leaf],
      inputNode: { slots: [{ name: 'seed', linkIds: [1] }] },
      getLink: () => ({ target_id: 'native-leaf', target_slot: 0 }),
    },
  };
  const cube: ComfyNode = {
    id: 'native-cube',
    properties: { sugarcubes_cube: { instance_id: 'stable-cube' } },
    subgraph: { _nodes: [face] },
  };
  const f = fixture([cube]);
  f.history.record(f.context, 1, [
    { path: 'stable-cube:sampling:sampler', name: 'seed', value: 42 },
  ]);
  expect(f.reuse.field(face, 'seed')?.readLast()).toBe(42);
  expect(f.reuse.field(leaf, 'seed')?.readLast()).toBe(42);
  f.reuse.field(face, 'seed')?.reuse();
  expect(leaf.widgets?.[0]?.value).toBe(42);
  expect(face.widgets?.[0]?.value).toBe(42);
});

test('restores and locks the linked master from a follower-only submission without breaking links', () => {
  const a = sampler('a'),
    b = sampler('b');
  const f = fixture([nativeCube('left', [a]), nativeCube('right', [b])]);
  f.relations.setOverrideEnabled('seed', false);
  const fields = f.relations.snapshot().fields;
  const master = fields.find((field) => field.cubeId === 'left');
  const follower = fields.find((field) => field.cubeId === 'right');
  if (!master || !follower) throw new Error('Missing seed field');
  f.relations.linkField(follower.id, master.id);
  f.history.record(f.context, 1, [{ path: 'right:b', name: 'seed', value: 42 }]);
  jest.mocked(f.graph.beforeChange!).mockClear();
  jest.mocked(f.graph.afterChange!).mockClear();
  f.reuse.field(b, 'seed')?.reuse();
  expect(a.widgets?.[0]?.value).toBe(42);
  expect(b.widgets?.[0]?.value).toBe(42);
  expect(f.relations.snapshot().state.localSeedModes[master.id]).toBe('fixed');
  expect(f.relations.snapshot().state.fieldLinks[follower.id]).toBe(master.id);
  expect(f.graph.beforeChange).toHaveBeenCalledTimes(1);
  expect(f.graph.afterChange).toHaveBeenCalledTimes(1);
  expect(f.error).not.toHaveBeenCalled();
});
