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
/** Prove canonical placement commits preserve native nodes, transactions and stale-edit safety. */
import { expect, jest, test } from '@jest/globals';
import { ComfyCubeGraphReorderHost } from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyCubeGraphReorderHost.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import { CubeGraphReorderService } from '../../../frontend/comfyui/ui/workflow/application/CubeGraphReorderService.js';
import { readCubeGraphAnalysis } from '../../../frontend/comfyui/ui/workflow/infrastructure/CubeGraphResponse.js';
import { graphResponse } from '../support/CubeGraphFixtures.js';

test('moves existing native vectors and emits exactly one undoable transaction', () => {
  const fixture = setup();
  const snapshot = fixture.host.snapshot();
  const node = fixture.nodes[0]!;
  const vector = node.pos;
  const widgets = node.widgets;
  expect(fixture.host.commit(snapshot, fixture.placements)).toBe(true);
  expect(node.pos).toBe(vector);
  expect(node.widgets).toBe(widgets);
  expect([...node.pos!]).toEqual([244, 0]);
  expect([...fixture.nodes[1]!.pos!]).toEqual([0, 0]);
  expect(fixture.beforeChange).toHaveBeenCalledTimes(1);
  expect(fixture.afterChange).toHaveBeenCalledTimes(1);
  expect(fixture.graphAfterChange).toHaveBeenCalledTimes(1);
  expect(fixture.setDirtyCanvas).toHaveBeenCalledWith(true, true);
  expect(fixture.onCommitted).toHaveBeenCalledWith(fixture.nodes);
});

test('reconciles linked configuration before the native reorder history entry closes', () => {
  let reconciled = false;
  const fixture = setup(() => {
    expect([...fixture.nodes[0]!.pos!]).toEqual([244, 0]);
    expect(fixture.afterChange).not.toHaveBeenCalled();
    reconciled = true;
  });
  fixture.afterChange.mockImplementation(() => expect(reconciled).toBe(true));
  expect(fixture.host.commit(fixture.host.snapshot(), fixture.placements)).toBe(true);
  expect(reconciled).toBe(true);
});

test('rejects a delayed placement after another geometry edit', () => {
  const fixture = setup();
  const snapshot = fixture.host.snapshot();
  fixture.nodes[0]!.pos![0] = 10;
  expect(fixture.host.commit(snapshot, fixture.placements)).toBe(false);
  expect(fixture.beforeChange).not.toHaveBeenCalled();
});

test.each([1 / 64, 200])(
  'keeps drag eligibility after a native height change of %s and analyzes fresh dimensions on drop',
  async (heightChange) => {
    const fixture = setup();
    const node = fixture.nodes[0]!;
    node.size = [220, 160];
    const analyze = jest.fn(async () => readCubeGraphAnalysis(graphResponse()));
    const reorder = jest.fn(
      async (
        snapshot: { serializedWorkflow: string },
        _current: readonly string[],
        order: readonly string[],
      ) => {
        expect(snapshot.serializedWorkflow).toContain(JSON.stringify([220, 160 + heightChange]));
        const response = graphResponse();
        for (const item of response.workflow.nodes)
          item.pos = [order.indexOf(item.properties.sugarcubes_cube.instance_id) * 244, 0];
        return readCubeGraphAnalysis(response);
      },
    );
    const service = new CubeGraphReorderService({
      host: fixture.host,
      gateway: { analyze, reorder },
    });
    await service.refresh();
    const beforeResize = fixture.host.snapshot();
    node.size[1] = 160 + heightChange;
    expect(fixture.host.isPlacementCurrent(beforeResize)).toBe(false);
    expect(service.analysis).not.toBeNull();
    expect(service.canMove('first', 1)).toBe(true);
    await service.move('first', 1);
    expect(analyze).toHaveBeenCalledTimes(2);
    expect(reorder).toHaveBeenCalledTimes(1);
    expect([...node.pos!]).toEqual([244, 0]);
    expect(service.pending).toBe(false);
  },
);

test('preserves intervening values and cache invalidations when committing geometry', () => {
  const fixture = setup();
  const snapshot = fixture.host.snapshot();
  fixture.nodes[0]!.widgets![0]!.value = 'edited';
  fixture.host.invalidateSnapshot();
  expect(fixture.host.isCurrent(snapshot)).toBe(true);
  expect(fixture.host.commit(snapshot, fixture.placements)).toBe(true);
  expect(fixture.nodes[0]!.widgets![0]!.value).toBe('edited');
  expect(fixture.beforeChange).toHaveBeenCalledTimes(1);
});

test('rejects replacement native objects with identical serialized identities and geometry', () => {
  const fixture = setup();
  const snapshot = fixture.host.snapshot();
  fixture.nodes[0] = { ...fixture.nodes[0]! };
  expect(fixture.host.snapshot().serializedWorkflow).toBe(snapshot.serializedWorkflow);
  expect(fixture.host.isCurrent(snapshot)).toBe(false);
  expect(fixture.host.commit(snapshot, fixture.placements)).toBe(false);
  expect(fixture.beforeChange).not.toHaveBeenCalled();
});

test('rejects a gesture after navigation away and back to the same native root', () => {
  const fixture = setup();
  const snapshot = fixture.host.snapshot();
  fixture.host.invalidateScope();
  expect(fixture.host.isCurrent(snapshot)).toBe(false);
  expect(fixture.host.commit(snapshot, fixture.placements)).toBe(false);
});

test('repeated permutations survive in-flight value updates and permission refreshes', async () => {
  const fixture = setup();
  const vectors = fixture.nodes.map((node) => node.pos);
  const reorder = jest.fn(
    async (_snapshot: unknown, _current: readonly string[], order: readonly string[]) => {
      fixture.nodes[0]!.widgets![0]!.value = 'edited while waiting';
      fixture.host.invalidateSnapshot();
      service.invalidate();
      await service.refresh();
      const response = graphResponse();
      for (const node of response.workflow.nodes)
        node.pos = [order.indexOf(node.properties.sugarcubes_cube.instance_id) * 244, 0];
      return readCubeGraphAnalysis(response);
    },
  );
  const service = new CubeGraphReorderService({
    host: fixture.host,
    gateway: {
      analyze: async (snapshot) => {
        const workflow: unknown = JSON.parse(snapshot.serializedWorkflow);
        return readCubeGraphAnalysis({ ...graphResponse(), workflow });
      },
      reorder,
    },
  });
  for (let index = 0; index < 10; index += 1) {
    const order = index % 2 === 0 ? ['second', 'first'] : ['first', 'second'];
    await service.reorder('first', order);
    expect(fixture.nodes.map((node) => node.pos![0])).toEqual(
      index % 2 === 0 ? [244, 0] : [0, 244],
    );
    expect(service.pending).toBe(false);
  }
  expect(reorder).toHaveBeenCalledTimes(10);
  expect(fixture.beforeChange).toHaveBeenCalledTimes(10);
  expect(fixture.afterChange).toHaveBeenCalledTimes(10);
  expect(fixture.nodes[0]!.widgets![0]!.value).toBe('edited while waiting');
  expect(fixture.nodes.map((node) => node.pos)).toEqual(vectors);
});

test('retains complete embedded definitions and live widget snapshots', () => {
  const fixture = setup();
  const parsed: unknown = JSON.parse(fixture.host.snapshot().serializedWorkflow);
  expect(parsed).toEqual(
    expect.objectContaining({
      definitions: expect.objectContaining({
        subgraphs: [
          expect.objectContaining({ id: 'definition', name: 'Embedded Cube', nodes: [] }),
        ],
      }),
    }),
  );
  expect(fixture.host.snapshot().serializedWorkflow).toContain('original');
});

test('restores every native vector when a later setter throws after writing', () => {
  const fixture = setup();
  fixture.nodes[1]!.setPos = (x, y) => {
    fixture.nodes[1]!.pos![0] = x;
    fixture.nodes[1]!.pos![1] = y;
    throw new Error('Native geometry failure');
  };
  expect(() => fixture.host.commit(fixture.host.snapshot(), fixture.placements)).toThrow(
    'Native geometry failure',
  );
  expect(fixture.nodes.map((node) => [...node.pos!])).toEqual([
    [0, 0],
    [244, 0],
  ]);
  expect(fixture.beforeChange).toHaveBeenCalledTimes(1);
  expect(fixture.afterChange).toHaveBeenCalledTimes(1);
  expect(fixture.onCommitted).not.toHaveBeenCalled();
});

test('rejects duplicate, nonfinite and foreign targets before starting history', () => {
  const fixture = setup();
  const snapshot = fixture.host.snapshot();
  expect(() =>
    fixture.host.commit(snapshot, [fixture.placements[0]!, fixture.placements[0]!]),
  ).toThrow('duplicate');
  expect(() =>
    fixture.host.commit(snapshot, [{ instanceId: 'first', nodeId: '1', position: [Infinity, 0] }]),
  ).toThrow('finite');
  expect(() =>
    fixture.host.commit(snapshot, [{ instanceId: 'other', nodeId: '1', position: [0, 0] }]),
  ).toThrow('identity');
  expect(fixture.beforeChange).not.toHaveBeenCalled();
});

test('declines mutations after navigation into a nested graph', () => {
  const fixture = setup();
  const snapshot = fixture.host.snapshot();
  fixture.leaveRoot();
  expect(fixture.host.available).toBe(false);
  expect(fixture.host.commit(snapshot, fixture.placements)).toBe(false);
  expect(fixture.beforeChange).not.toHaveBeenCalled();
});

test('does not create an undo entry for unchanged placements', () => {
  const fixture = setup();
  expect(
    fixture.host.commit(fixture.host.snapshot(), [
      { instanceId: 'first', nodeId: '1', position: [0, 0] },
    ]),
  ).toBe(true);
  expect(fixture.beforeChange).not.toHaveBeenCalled();
  expect(fixture.onCommitted).not.toHaveBeenCalled();
});

/** Supply native vectors and serializers rather than mocking coordinate or snapshot behavior. */
function setup(reconcileOrder: () => void = () => undefined) {
  const nodes: ComfyNode[] = ['first', 'second'].map((instanceId, index) => ({
    id: index + 1,
    pos: new Float32Array([index * 244, 0]),
    properties: { sugarcubes_cube: { instance_id: instanceId } },
    widgets: [{ name: 'prompt', value: 'original' }],
  }));
  const graphAfterChange = jest.fn();
  const graph: ComfyGraph = {
    _nodes: nodes,
    afterChange: graphAfterChange,
    _subgraphs: new Map([['definition', { id: 'definition', name: 'Embedded Cube', nodes: [] }]]),
    serialize: () => ({
      nodes: nodes.map((node) => ({
        id: node.id,
        pos: [...node.pos!],
        size: node.size ? [...node.size] : undefined,
        properties: node.properties,
        widgets_values: node.widgets?.map((widget) => widget.value),
      })),
    }),
  };
  const beforeChange = jest.fn();
  const afterChange = jest.fn();
  const setDirtyCanvas = jest.fn();
  const onCommitted = jest.fn();
  let isRoot = true;
  const host = new ComfyCubeGraphReorderHost({
    graph,
    history: { beforeChange, afterChange, setDirtyCanvas },
    isCurrentRoot: () => isRoot,
    onCommitted,
    reconcileOrder,
  });
  const placements = [
    { instanceId: 'first', nodeId: '1', position: [244, 0] as const },
    { instanceId: 'second', nodeId: '2', position: [0, 0] as const },
  ];
  return {
    host,
    nodes,
    placements,
    beforeChange,
    afterChange,
    graphAfterChange,
    setDirtyCanvas,
    onCommitted,
    leaveRoot: () => {
      isRoot = false;
    },
  };
}
