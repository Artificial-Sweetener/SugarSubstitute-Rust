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
/** Prove canonical permissions refresh at native graph boundaries and ignore unrelated DOM activity. */
import { expect, jest, test } from '@jest/globals';
import { createComfyCubeReorder } from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyCubeReorderComposition.js';
import { readCubeGraphAnalysis } from '../../../frontend/comfyui/ui/workflow/infrastructure/CubeGraphResponse.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';
import type { ObservableNativeSubgraph } from '../../../frontend/comfyui/ui/surface/NativeSubgraphChangeObserver.js';
import { graphResponse } from '../support/CubeGraphFixtures.js';

test('reads cached permissions without serializing for every row and ignores popup activity', async () => {
  const fixture = setup();
  await fixture.settle();
  const serializations = fixture.serialize.mock.calls.length;
  const requests = fixture.analyze.mock.calls.length;
  for (let index = 0; index < 50; index += 1)
    expect(fixture.reorder.service.canMove('first', 1)).toBe(true);
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  document.body.append(popup);
  popup.textContent = 'Unrelated native choices';
  await fixture.settle();
  popup.remove();
  expect(fixture.serialize).toHaveBeenCalledTimes(serializations);
  expect(fixture.analyze).toHaveBeenCalledTimes(requests);
  fixture.reorder.dispose();
});

test('publishes permission changes without inventing a Cube catalog mutation', async () => {
  const fixture = setup();
  await fixture.settle();
  const inventory = jest.spyOn(fixture.nodes, 'list');
  const changed = jest.fn();
  const release = fixture.nodes.subscribe(changed);
  fixture.reorder.service.invalidate();
  await fixture.reorder.service.refresh();
  expect(inventory).not.toHaveBeenCalled();
  expect(changed).not.toHaveBeenCalled();
  release();
  fixture.reorder.dispose();
});

test('coalesces no-op native notifications without reanalysis and preserves the host callback', async () => {
  const fixture = setup();
  await fixture.settle();
  fixture.graph.add?.({});
  fixture.graph.remove?.({});
  fixture.graph.onAfterChange?.(fixture.graph);
  await fixture.settle();
  expect(fixture.analyze).toHaveBeenCalledTimes(1);
  expect(fixture.hostAfter).toHaveBeenCalledTimes(1);
  fixture.reorder.dispose();
  expect(fixture.graph.onAfterChange).toBe(fixture.hostAfter);
  expect(fixture.graph.add).toBe(fixture.hostAdd);
  expect(fixture.graph.remove).toBe(fixture.hostRemove);
});

test('refreshes canonical facts after real native membership and Cube geometry changes', async () => {
  const fixture = setup();
  await fixture.settle();
  const node = {
    id: 1,
    pos: [0, 0],
    size: [220, 160],
    properties: { sugarcubes_cube: { instance_id: 'first' } },
  };
  fixture.graph._nodes = [node];
  fixture.graph.onAfterChange?.(fixture.graph);
  await fixture.settle();
  expect(fixture.analyze).toHaveBeenCalledTimes(2);
  node.pos[0] = 10;
  fixture.graph.onAfterChange?.(fixture.graph);
  await fixture.settle();
  expect(fixture.analyze).toHaveBeenCalledTimes(3);
  fixture.reorder.dispose();
});

test('retires permissions on root navigation and restores them when returning', async () => {
  const fixture = setup();
  await fixture.settle();
  fixture.navigate(false);
  expect(fixture.reorder.service.canMove('first', 1)).toBe(false);
  await fixture.settle();
  expect(fixture.analyze).toHaveBeenCalledTimes(1);
  fixture.navigate(true);
  await fixture.settle();
  expect(fixture.reorder.service.canMove('first', 1)).toBe(true);
  expect(fixture.analyze).toHaveBeenCalledTimes(2);
  fixture.reorder.dispose();
});

test('reports canonical analysis failures and disposes queued refresh work', async () => {
  const fixture = setup();
  fixture.analyze.mockRejectedValue(new Error('Graph API unavailable'));
  await fixture.settle();
  expect(fixture.reportError).toHaveBeenCalledWith(
    expect.objectContaining({ message: 'Graph API unavailable' }),
  );
  fixture.graph.add?.({});
  fixture.reorder.dispose();
  await fixture.settle();
  expect(fixture.analyze).toHaveBeenCalledTimes(1);
  expect(fixture.navigation.size).toBe(0);
});

/** Build a real native callback chain with isolated graph, catalog and navigation owners. */
function setup() {
  const response = graphResponse();
  const serialize = jest.fn(() => response.workflow);
  const hostAfter = jest.fn();
  const hostAdd = jest.fn();
  const hostRemove = jest.fn();
  const graph: ComfyGraph & ObservableNativeSubgraph = {
    serialize,
    add: hostAdd,
    remove: hostRemove,
    onAfterChange: hostAfter,
  };
  const analyze = jest.fn(async () => readCubeGraphAnalysis(response));
  const reportError = jest.fn();
  const navigation = new Set<() => void>();
  let isRoot = true;
  const nodes = new CubeNodeCatalog();
  const reorder = createComfyCubeReorder({
    graph,
    nodes,
    history: { beforeChange() {}, afterChange() {}, setDirtyCanvas() {} },
    gateway: { analyze, reorder: async () => readCubeGraphAnalysis(response) },
    graphChanges: {
      subscribe(listener) {
        navigation.add(listener);
        return () => {
          navigation.delete(listener);
        };
      },
    },
    isCurrentRoot: () => isRoot,
    reportError,
    reconcileOrder() {},
  });
  return {
    graph,
    nodes,
    reorder,
    serialize,
    analyze,
    reportError,
    navigation,
    hostAfter,
    hostAdd,
    hostRemove,
    navigate(root: boolean) {
      isRoot = root;
      for (const listener of navigation) listener();
    },
    async settle() {
      for (let index = 0; index < 8; index += 1) await Promise.resolve();
    },
  };
}
