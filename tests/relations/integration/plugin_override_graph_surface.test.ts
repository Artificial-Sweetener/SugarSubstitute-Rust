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
/** Prove the Plugin Mode card uses one unconnected native graph node and preserves workflow data. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeOverrideGraphSurface } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeOverrideGraphSurface.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

/** Model only the external LiteGraph constructor and DOM-widget mounting boundary. */
function fixture() {
  const canvas = document.createElement('div');
  document.body.append(canvas);
  const existing: ComfyNode = { id: 1, type: 'KSampler', pos: [80, 90] };
  const previousSerialize = jest.fn<(data: unknown) => void>();
  const graph: ComfyGraph = {
    _nodes: [existing],
    extra: { retained: true },
    onSerialize: previousSerialize,
    add: (node) => {
      node.id = 2;
      graph._nodes?.push(node);
    },
    remove: (node) => {
      graph._nodes = graph._nodes?.filter((item) => item !== node) ?? [];
    },
    setDirtyCanvas: jest.fn(),
  };
  class Node {
    readonly title: string;
    /** Retain the title passed through the native base constructor. */
    constructor(title = '') {
      this.title = title;
    }
    pos = [0, 0];
    size = [400, 100];
    widgets: UnknownRecord[] = [];
    readonly flags = { collapsed: false };
    /** Expose the same read-only native capability that guards collapse affordances. */
    get collapsible(): boolean {
      return !this.flags.collapsed;
    }

    /** Mirror the native widget's ownership of its supplied DOM element. */
    addDOMWidget(name: string, type: string, element: HTMLElement, options: UnknownRecord) {
      canvas.append(element);
      const widget = { name, type, element, options };
      this.widgets.push(widget);
      return widget;
    }
    /** Record native resizing without adding a parallel geometry implementation. */
    setSize(size: number[]) {
      this.size = size;
    }
  }
  const runtime = {
    LGraphNode: Node,
    registerNodeType: jest.fn<(type: string, node: new () => ComfyNode) => void>(),
  };
  const surface = new NativeOverrideGraphSurface(
    document,
    () => ({ graph, canvas: { ds: { scale: 1, offset: [0, 0] } } }),
    () => runtime,
  );
  const root = document.createElement('div');
  const input = document.createElement('input');
  input.value = '42';
  root.append(input);
  return { surface, graph, root, input, existing, previousSerialize, runtime };
}

afterEach(() => document.body.replaceChildren());

test('the same override controls float in a native node with no connections', () => {
  const f = fixture();
  expect(f.surface.attach(f.root)).toBe(true);
  const node = f.graph._nodes?.[1];
  expect(node?.title).toBe('Global Overrides');
  expect(node?.isVirtualNode).toBe(true);
  expect(node?.inputs).toEqual([]);
  expect(node?.outputs).toEqual([]);
  expect(node?.collapsible).toBe(false);
  expect(node?.clonable).toBe(false);
  expect(node?.block_delete).toBe(true);
  const RegisteredNode = f.runtime.registerNodeType.mock.calls[0]?.[1];
  expect(RegisteredNode && new RegisteredNode().isVirtualNode).toBe(true);
  expect(f.root.closest('.sc-overrides-graph-content')).not.toBeNull();
  expect(f.surface.attach(f.root)).toBe(true);
  expect(f.graph._nodes).toHaveLength(2);
  expect(f.root.querySelector('input')).toBe(f.input);
  expect(f.input.value).toBe('42');
  f.surface.detach();
  expect(f.graph._nodes).toEqual([f.existing]);
  expect(f.root.querySelector('input')).toBe(f.input);
  expect(f.graph.onSerialize).toBe(f.previousSerialize);
});

test('serialization saves only graph placement and retains executable nodes and earlier hooks', () => {
  const f = fixture();
  f.surface.attach(f.root);
  const node = f.graph._nodes?.[1];
  if (!node) throw new Error('Expected the presentation node');
  node.pos = [320, 480];
  node.size = [420, 180];
  const data: UnknownRecord = { nodes: [{ id: 1 }, { id: 2 }], extra: { retained: true } };
  const serialize = f.graph.onSerialize;
  if (typeof serialize !== 'function') throw new Error('Expected serialization hook');
  serialize.call(f.graph, data);
  expect(f.previousSerialize).toHaveBeenCalledWith(data);
  expect(data.nodes).toEqual([{ id: 1 }]);
  expect(data.extra).toMatchObject({
    retained: true,
    sugarcubes_override_card: { x: 320, y: 480, width: 420 },
  });
  f.surface.detach();
  f.surface.attach(f.root);
  expect(f.graph._nodes?.[1]?.pos).toEqual([320, 480]);
  f.surface.beforeConfigureGraph();
  f.graph.extra = {};
  f.surface.attach(f.root);
  expect(f.graph._nodes?.[1]?.pos).not.toEqual([320, 480]);
  f.surface.detach();
});

test('unsupported graph capabilities fail before changing workflow nodes', () => {
  const f = fixture();
  const unsupported = new NativeOverrideGraphSurface(
    document,
    () => ({ graph: f.graph }),
    () => ({}),
  );
  expect(() => unsupported.attach(f.root)).toThrow(/DOM widget/);
  expect(f.graph._nodes).toEqual([f.existing]);
  f.surface.detach();
});

test('retains the native graph endpoint and the same controls while masonry borrows the card', () => {
  const f = fixture();
  f.surface.attach(f.root);
  const node = f.graph._nodes?.[1];
  const slot = document.createElement('div');
  document.body.append(slot);
  expect(f.surface.showInMasonry(f.root, slot)).toBe(true);
  expect(f.graph._nodes?.[1]).toBe(node);
  expect(slot.firstElementChild).toBe(f.root);
  expect(f.root.hasAttribute('data-sc-masonry-supplement')).toBe(true);
  expect(document.querySelector('[data-sc-masonry-supplement-source]')).not.toBeNull();
  f.surface.showInMasonry(f.root, slot);
  expect(f.graph._nodes).toHaveLength(2);
  f.surface.attach(f.root);
  expect(f.graph._nodes?.[1]).toBe(node);
  expect(f.root.closest('.sc-overrides-graph-content')).not.toBeNull();
  expect(f.root.querySelector('input')).toBe(f.input);
  f.surface.detach();
  expect(f.root.hasAttribute('data-sc-masonry-supplement')).toBe(false);
});

test('settles retained graph height after fields mount in masonry without following animation frames', () => {
  const f = fixture();
  const fields = document.createElement('div');
  fields.className = 'sc-overrides-fields';
  fields.append(f.input);
  f.root.append(fields);
  let fieldHeight = 120;
  Object.defineProperty(fields, 'scrollHeight', { get: () => fieldHeight });
  const slot = document.createElement('div');
  document.body.append(slot);
  f.surface.showInMasonry(f.root, slot);
  const node = f.graph._nodes?.[1];
  expect(node?.size?.[1]).toBe(160);
  f.root.dataset.scGeometryMoving = '';
  fieldHeight = 180;
  f.surface.showInMasonry(f.root, slot);
  expect(node?.size?.[1]).toBe(160);
  delete f.root.dataset.scGeometryMoving;
  f.surface.showInMasonry(f.root, slot);
  expect(node?.size?.[1]).toBe(220);
  f.surface.detach();
});

test('keeps presentation nodes out of native removal history snapshots', () => {
  const f = fixture();
  f.surface.attach(f.root);
  const remove = f.graph.remove;
  const snapshots: unknown[] = [];
  f.graph.remove = (node) => {
    const data: UnknownRecord = {
      nodes: f.graph._nodes?.map((item) => ({ id: item.id })),
      extra: {},
    };
    const serialize = f.graph.onSerialize;
    if (typeof serialize === 'function') serialize.call(f.graph, data);
    snapshots.push(data.nodes);
    remove?.(node);
  };
  f.surface.detach();
  expect(snapshots).toEqual([[{ id: 1 }]]);
  expect(f.graph.onSerialize).toBe(f.previousSerialize);
});
