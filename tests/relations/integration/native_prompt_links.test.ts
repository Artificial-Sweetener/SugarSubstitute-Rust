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
/** Prove semantic prompt links through native Cube ownership and workflow order. */
import { expect, test } from '@jest/globals';
import { WorkflowRelations } from '../../../frontend/comfyui/ui/relations/application/WorkflowRelations.js';
import { NativeRelationWorkflowPort } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationWorkflowPort.js';
import { NativeRelationInventory } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationInventory.js';
import { compatibleNodes } from '../../../frontend/comfyui/ui/relations/domain/RelationResolver.js';
import { projectWorkflowMasonry } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import type { NativeGraphNode } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';
import { nativeCube } from '../support/native_cube.js';

/** Identify the same string node by prompt role instead of treating every multiline value as equivalent. */
function prompt(id: string, role: 'Positive' | 'Negative', value: string): NativeGraphNode {
  return {
    id,
    type: 'StringConstant',
    title: `${role} Prompt`,
    mode: 0,
    pos: [0, 0],
    size: [330, 140],
    properties: {},
    inputs: [],
    outputs: [{ name: 'STRING', type: 'STRING' }],
    widgets: [{ name: 'value', type: 'customtext', value, options: { multiline: true } }],
    connect() {},
  };
}

test('links positive and negative Cube prompts separately, propagates text, and retains masters after reorder', () => {
  const aPositive = prompt('positive', 'Positive', 'a positive');
  const aNegative = prompt('negative', 'Negative', 'a negative');
  const bPositive = prompt('positive', 'Positive', 'b positive');
  const bNegative = prompt('negative', 'Negative', 'b negative');
  const a = nativeCube('a', [aPositive, aNegative], 0);
  const b = nativeCube('b', [bPositive, bNegative], 800);
  const graph: ComfyGraph = { _nodes: [b, a], extra: {} };
  const port = new NativeRelationWorkflowPort(
    () => graph,
    document,
    console,
    () => null,
  );
  const relations = new WorkflowRelations(port);
  const snapshot = relations.reconcile();
  const node = (cubeId: string, role: string) =>
    snapshot.nodes.find((n) => n.cubeId === cubeId && n.promptRole === role)!;
  expect(snapshot.nodes.filter((n) => n.linkable)).toHaveLength(4);
  expect(compatibleNodes(node('a', 'positive'), node('b', 'negative'))).toBe(false);
  expect(snapshot.state.nodeLinks[node('b', 'positive').id]).toBe(node('a', 'positive').id);
  expect(snapshot.state.nodeLinks[node('b', 'negative').id]).toBe(node('a', 'negative').id);
  expect(bPositive.widgets?.[0]?.value).toBe('a positive');
  expect(bNegative.widgets?.[0]?.value).toBe('a negative');
  relations.setFieldValue(node('a', 'positive').fields[0]!.id, '<b>updated prompt</b>');
  expect(bPositive.widgets?.[0]?.value).toBe('<b>updated prompt</b>');
  expect(bNegative.widgets?.[0]?.value).toBe('a negative');
  a.pos = [1000, 0];
  b.pos = [0, 0];
  const reordered = relations.reconcile();
  expect(reordered.nodes.map((n) => n.cubeId)).toEqual(['b', 'b', 'a', 'a']);
  expect(reordered.state.nodeLinks[node('b', 'positive').id]).toBe(node('a', 'positive').id);
  expect(projectWorkflowMasonry(graph, [a, b]).map((section) => section.cube)).toEqual([b, a]);
  expect(bPositive.widgets?.[0]?.value).toBe('<b>updated prompt</b>');
  relations.linkNode(node('b', 'positive').id, null);
  expect(bPositive.widgets?.[0]?.value).toBe('b positive');
});

test('excludes connected prompt widgets and resource inheritance while honoring accepted workflow edges', () => {
  const connected = prompt('connected', 'Positive', 'upstream owns this');
  connected.inputs = [{ name: 'value', widget: { name: 'value' }, link: 0 }];
  const loader: NativeGraphNode = {
    id: 'loader',
    type: 'ModelLoader',
    title: 'Positive Prompt model loader',
    pos: [0, 0],
    size: [330, 140],
    properties: {},
    inputs: [],
    outputs: [{ type: 'MODEL' }],
    widgets: [{ name: 'model', type: 'customtext', value: 'model', options: { multiline: true } }],
    connect() {},
  };
  const a = nativeCube('a', [connected, loader], 0);
  const b = nativeCube('b', [prompt('positive', 'Positive', 'b')], 800);
  const graph: ComfyGraph = { _nodes: [a, b] };
  const connections = [{ source: 'b', target: 'a' }];
  const inventory = new NativeRelationInventory(console).read(graph, connections);
  expect(inventory.nodes.filter((n) => n.linkable).map((n) => n.cubeId)).toEqual(['b']);
  expect(inventory.fields.some((f) => f.contract.value === 'upstream owns this')).toBe(false);
  expect(inventory.nodes.filter((n) => n.cubeId === 'a').every((n) => !n.linkable)).toBe(true);
  expect(projectWorkflowMasonry(graph, [a, b], connections).map((s) => s.cube)).toEqual([b, a]);
});
