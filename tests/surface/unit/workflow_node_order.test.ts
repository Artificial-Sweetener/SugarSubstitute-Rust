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
/** Prove shared reading order and accepted implicit graph dependencies. */
import { expect, jest, test } from '@jest/globals';
import { orderWorkflowNodes } from '../../../frontend/comfyui/ui/surface/WorkflowNodeOrder.js';
import { proximityWorkflowConnections } from '../../../frontend/comfyui/ui/overlays/proximity/ProximityWorkflowConnections.js';
import type { ProximityMatch } from '../../../frontend/comfyui/ui/overlays/proximity/ProximityModel.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';

test('uses position for independent nodes and honors native and accepted implicit dependencies', () => {
  const left = { id: 'left', pos: [0, 0] };
  const right = { id: 'right', pos: [100, 0], inputs: [{ link: 1 }] };
  const graph: ComfyGraph = { _nodes: [right, left] };
  expect(orderWorkflowNodes(graph)).toEqual([left, right]);
  graph.links = [{ id: 1, origin_id: 'left', target_id: 'right' }];
  left.pos = [200, 0];
  expect(orderWorkflowNodes(graph)).toEqual([left, right]);
  graph.links = [];
  expect(orderWorkflowNodes(graph)).toEqual([right, left]);
  expect(orderWorkflowNodes(graph, [{ source: 'left', target: 'right' }])).toEqual([left, right]);
});

test('keeps Cube order spatial without moving loose-node dependency slots', () => {
  const first = nativeCube('first', 200);
  const second = nativeCube('second', 100);
  second.inputs = [{ link: 1 }];
  const loose = { id: 'loose', pos: [300, 0] };
  const graph: ComfyGraph = {
    _nodes: [first, second, loose],
    links: [{ id: 1, origin_id: 'first', target_id: 'second' }],
  };
  expect(orderWorkflowNodes(graph)).toEqual([second, loose, first]);
  first.pos = [0, 0];
  expect(orderWorkflowNodes(graph)).toEqual([first, loose, second]);
});

/** Supply a recognized native Cube wrapper without editable implementation fields. */
function nativeCube(id: string, x: number) {
  return {
    id,
    pos: [x, 0],
    size: [720, 480],
    inputs: [] as Array<{ link: number }>,
    outputs: [],
    properties: { sugarcubes_kind: 'cube', sugarcubes_cube: { instance_id: id } },
    subgraph: { id: `definition-${id}`, name: 'Cube', _nodes: [], inputs: [], outputs: [] },
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}

test('projects accepted proximity edges once and excludes unresolved endpoints while retaining zero ids', () => {
  const graph: ComfyGraph = { _nodes: [] };
  const match: ProximityMatch = {
    outputId: 0,
    inputId: 'target',
    outputSlot: 0,
    inputSlot: 0,
    inputName: 'image',
    outputPos: [0, 0],
    inputPos: [1, 0],
    originId: 0,
    originSlot: 0,
    promptTargets: [],
    distance: 1,
  };
  const resolveExecutionMatches = jest.fn(() => [
    match,
    { ...match, outputId: undefined },
    { ...match, inputId: undefined },
  ]);
  expect(proximityWorkflowConnections({ resolveExecutionMatches }, graph)).toEqual([
    { source: 0, target: 'target' },
  ]);
  expect(resolveExecutionMatches).toHaveBeenCalledTimes(1);
  expect(resolveExecutionMatches).toHaveBeenCalledWith(graph);
});
