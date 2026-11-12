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
import { afterEach, expect, jest, test } from '@jest/globals';
import { WorkflowRelations } from '../../../frontend/comfyui/ui/relations/application/WorkflowRelations.js';
import { NativeRelationWorkflowPort } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationWorkflowPort.js';
import { NativeRelationInventory } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationInventory.js';
import { compatibleNodes } from '../../../frontend/comfyui/ui/relations/domain/RelationResolver.js';
import { projectWorkflowMasonry } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import type { NativeGraphNode } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';
import { nativeCube } from '../support/native_cube.js';
import {
  CubeRelationPresenter,
  type RelationCardView,
} from '../../../frontend/comfyui/ui/relations/presentation/CubeRelationPresenter.js';
import { ComfyCubeGraphReorderHost } from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyCubeGraphReorderHost.js';
import { ensureRelationStyles } from '../../../frontend/comfyui/ui/relations/presentation/RelationStyles.js';

afterEach(() => {
  document.body.replaceChildren();
  document.getElementById('sc-relation-styles')?.remove();
});

test('publishes one scoped native refresh after value and visibility changes in the same turn', async () => {
  const a = nativeCube('a', [prompt('positive', 'Positive', 'a')], 0);
  const b = nativeCube('b', [prompt('positive', 'Positive', 'b')], 800);
  const graph: ComfyGraph = { _nodes: [a, b], extra: {} };
  const port = new NativeRelationWorkflowPort(
    () => graph,
    document,
    console,
    () => null,
  );
  const inventory = port.inventory();
  const listener = jest.fn();
  document.addEventListener('sugarcubes-native-node-card-change', listener);
  try {
    const first = inventory.nodes[0]!.id;
    const second = inventory.nodes[1]!.id;
    port.refresh([first]);
    port.refresh([first, second]);
    expect(listener).not.toHaveBeenCalled();
    await Promise.resolve();
    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledWith(
      expect.objectContaining({
        detail: { kind: 'nodes', nodes: [a.subgraph._nodes[0], b.subgraph._nodes[0]] },
      }),
    );
  } finally {
    document.removeEventListener('sugarcubes-native-node-card-change', listener);
  }
});

test('discards a queued native refresh when its workflow is replaced', async () => {
  const a = nativeCube('a', [prompt('positive', 'Positive', 'a')], 0);
  let graph: ComfyGraph = { _nodes: [a], extra: {} };
  const port = new NativeRelationWorkflowPort(
    () => graph,
    document,
    console,
    () => null,
  );
  const nodeId = port.inventory().nodes[0]!.id;
  const listener = jest.fn();
  document.addEventListener('sugarcubes-native-node-card-change', listener);
  try {
    port.refresh([nodeId]);
    graph = { _nodes: [], extra: {} };
    await Promise.resolve();
    expect(listener).not.toHaveBeenCalled();
  } finally {
    document.removeEventListener('sugarcubes-native-node-card-change', listener);
  }
});

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

test.each([false, true])(
  'promotes both prompt inputs and retains later selectors with native visibility ownership %s',
  (nativeWidgetVisibility) => {
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
    ensureRelationStyles(document);
    const views: RelationCardView[] = snapshot.nodes.map((contract) => {
      const root = document.createElement('article');
      const header = document.createElement('header');
      const title = document.createElement('div');
      title.textContent = contract.label;
      header.append(title);
      const row = document.createElement('div');
      row.append(document.createElement('textarea'));
      root.append(header, row);
      document.body.append(root);
      return {
        nodeId: contract.id,
        surface: {
          nativeWidgetVisibility,
          root,
          header,
          rows: new Map([['value', row]]),
          canvas: null,
          canvasFields: new Map(),
          canvasTitle: false,
        },
      };
    });
    const presenter = new CubeRelationPresenter(document, relations, () => views, jest.fn(), {
      mount: () => {
        throw new Error('Prompt rows use native title choices');
      },
      mountChoice: () => ({ update: jest.fn(), dispose: jest.fn() }),
    });
    presenter.reconcile();
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
    expect(reordered.state.nodeLinks[node('b', 'positive').id]).toBeNull();
    expect(reordered.state.nodeLinks[node('b', 'negative').id]).toBeNull();
    expect(reordered.state.nodeLinks[node('a', 'positive').id]).toBe(node('b', 'positive').id);
    expect(reordered.state.nodeLinks[node('a', 'negative').id]).toBe(node('b', 'negative').id);
    presenter.reconcile();
    for (const role of ['positive', 'negative']) {
      const first = views.find((view) => view.nodeId === node('b', role).id)!;
      const later = views.find((view) => view.nodeId === node('a', role).id)!;
      expect(first.surface.rows.get('value')?.hasAttribute('data-sc-relation-hidden')).toBe(false);
      expect(first.surface.rows.get('value')?.querySelector('textarea')).not.toBeNull();
      expect(first.surface.root.querySelector<HTMLElement>('.sc-node-link')?.hidden).toBe(true);
      expect(later.surface.root.querySelector<HTMLElement>('.sc-node-link')?.hidden).toBe(false);
      expect(later.surface.rows.get('value')?.hasAttribute('data-sc-relation-hidden')).toBe(
        !nativeWidgetVisibility,
      );
    }
    expect(projectWorkflowMasonry(graph, [a, b]).map((section) => section.cube?.id)).toEqual([
      'b',
      'a',
    ]);
    expect(bPositive.widgets?.[0]?.value).toBe('<b>updated prompt</b>');
    relations.linkNode(node('a', 'positive').id, null);
    presenter.reconcile();
    expect(aPositive.widgets?.[0]?.value).toBe('<b>updated prompt</b>');
    const independent = views.find((view) => view.nodeId === node('a', 'positive').id)!;
    expect(independent.surface.rows.get('value')?.hasAttribute('data-sc-relation-hidden')).toBe(
      false,
    );
    expect(independent.surface.root.querySelector<HTMLElement>('.sc-node-link')?.hidden).toBe(
      false,
    );
    presenter.dispose();
  },
);

test('excludes connected prompt widgets and resource inheritance while retaining Cube placement order', () => {
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
  expect(projectWorkflowMasonry(graph, [a, b], connections).map((s) => s.cube?.id)).toEqual([
    'a',
    'b',
  ]);
});

test('records prompt promotion and Cube placement in one native history transaction', () => {
  const aPrompt = prompt('positive', 'Positive', 'shared contents');
  const bPrompt = prompt('positive', 'Positive', 'dormant contents');
  const a = nativeCube('a', [aPrompt], 0);
  const b = nativeCube('b', [bPrompt], 800);
  const beforeChange = jest.fn();
  const afterChange = jest.fn();
  const graph: ComfyGraph = {
    _nodes: [a, b],
    extra: {},
    beforeChange,
    afterChange,
    serialize: () => ({
      nodes: [a, b].map((cube) => ({ id: cube.id, pos: cube.pos, properties: cube.properties })),
    }),
  };
  const port = new NativeRelationWorkflowPort(
    () => graph,
    document,
    console,
    () => null,
  );
  const relations = new WorkflowRelations(port);
  const original = relations.reconcile();
  const aId = original.nodes.find((node) => node.cubeId === 'a')!.id;
  const bId = original.nodes.find((node) => node.cubeId === 'b')!.id;
  beforeChange.mockClear();
  afterChange.mockClear();
  const entryBefore = jest.fn();
  const entryAfter = jest.fn(() => {
    expect(port.load().nodeLinks[bId]).toBeNull();
    expect(port.load().nodeLinks[aId]).toBe(bId);
    expect(bPrompt.widgets?.[0]?.value).toBe('shared contents');
    expect(a.pos?.[0]).toBe(800);
    expect(b.pos?.[0]).toBe(0);
  });
  const host = new ComfyCubeGraphReorderHost({
    graph,
    isCurrentRoot: () => true,
    history: { beforeChange: entryBefore, afterChange: entryAfter, setDirtyCanvas() {} },
    reconcileOrder: () => port.joinTransaction(() => relations.reconcile()),
    onCommitted() {},
  });
  expect(
    host.commit(host.snapshot(), [
      { instanceId: 'a', nodeId: 'a', position: [800, 0] },
      { instanceId: 'b', nodeId: 'b', position: [0, 0] },
    ]),
  ).toBe(true);
  expect(entryBefore).toHaveBeenCalledTimes(1);
  expect(entryAfter).toHaveBeenCalledTimes(1);
  expect(beforeChange).not.toHaveBeenCalled();
  expect(afterChange).toHaveBeenCalledTimes(1);
  const fail = new Error('external mutation failed');
  expect(() =>
    port.joinTransaction(() => {
      throw fail;
    }),
  ).toThrow(fail);
  port.transaction(() => undefined);
  expect(beforeChange).toHaveBeenCalledTimes(1);
  expect(afterChange).toHaveBeenCalledTimes(2);
});
