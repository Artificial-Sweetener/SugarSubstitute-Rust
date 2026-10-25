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
/** Prove native association updates, workflow isolation and zero discovery from source or popup activity. */
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import { associationGraph, associationNodes } from '../support/association_graph.js';
import { sourceMaskFixture } from '../support/source_mask_fixture.js';
import { ComfyInputAssociations } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputAssociations.js';
import { ComfyInputBindings } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBindings.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

beforeAll(() =>
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: randomUUID }),
);
/** Settle native coalescing without timers or DOM-based discovery. */
async function settled(): Promise<void> {
  for (let index = 0; index < 80; index++) await Promise.resolve();
}
/** Keep native graph and canvas boundaries controlled while using the real relationship and binding registries. */
function fixture() {
  const f = sourceMaskFixture();
  f.loaders.define({ LoadImage: {} });
  const native = associationGraph(),
    nodes = associationNodes();
  native.add(nodes.image, nodes.mask, nodes.consumer);
  native.connect(1, nodes.image, 0, nodes.consumer, 0);
  native.connect(2, nodes.mask, 0, nodes.consumer, 1);
  let root: ComfyGraph = native.graph;
  const bindings = new ComfyInputBindings(f.loaders, f.ports),
    owner = new ComfyInputAssociations(f.loaders, bindings, () => root, f.report, {
      warn: jest.fn(),
    });
  owner.start();
  return {
    ...f,
    ...native,
    ...nodes,
    owner,
    bindings,
    root: (graph: ComfyGraph) => {
      root = graph;
    },
    association: () => owner.relationships.read(owner.key(nodes.mask, 'image')),
    dispose: () => {
      owner.dispose();
      bindings.clear();
      f.previews.dispose();
    },
  };
}

test('shares one relationship authority with source lookup and reverse queries before any card is mounted', async () => {
  const f = fixture();
  expect(f.association()).toMatchObject({
    kind: 'associated',
    image: f.owner.key(f.image, 'image'),
  });
  const source = f.owner.image(f.mask, 'image').read();
  expect(source).toBe(f.bindings.get(f.image, f.loaders.read('LoadImage')!));
  expect(f.owner.binding(f.owner.key(f.image, 'image'))).toBe(source);
  expect(f.owner.relationships.masksFor(f.owner.key(f.image, 'image'))).toEqual([
    f.owner.key(f.mask, 'image'),
  ]);
  await settled();
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('updates programmatic connections and ambiguity without rebuilding native membership', async () => {
  const f = fixture(),
    changed = jest.fn();
  f.owner.image(f.mask, 'image').subscribe(changed);
  const initial = f.owner.diagnostics();
  f.disconnect(1);
  await settled();
  expect(f.association().kind).toBe('missing');
  f.connect(3, f.image, 0, f.consumer, 0);
  await settled();
  expect(f.association().kind).toBe('associated');
  const other: ComfyNode = { ...associationNodes().image, id: 1000004 };
  f.add(other);
  f.consumer.inputs!.push({ name: 'other', type: 'IMAGE', link: null });
  f.connect(4, other, 0, f.consumer, 2);
  await settled();
  expect(f.association().kind).toBe('ambiguous');
  const membership = f.owner.diagnostics().membershipReads;
  f.disconnect(4);
  await settled();
  expect(f.association().kind).toBe('associated');
  expect(f.owner.diagnostics().membershipReads).toBe(membership);
  expect(membership).toBeGreaterThan(initial.membershipReads);
  expect(changed).toHaveBeenCalledTimes(4);
  f.dispose();
});

test('keeps a connected image input with an unknown provider from authorizing a guessed association', async () => {
  const f = fixture(),
    unknown: ComfyNode = {
      id: 1000004,
      type: 'UnknownProvider',
      outputs: [{ name: 'output', type: '*', links: [] }],
    };
  f.add(unknown);
  f.consumer.inputs!.push({ name: 'otherImage', type: 'IMAGE', link: null });
  f.connect(3, unknown, 0, f.consumer, 2);
  await settled();
  expect(f.association().kind).toBe('ambiguous');
  f.dispose();
});

test('source values, channel edits, native value transactions and unrelated popups perform zero association discovery', async () => {
  const f = fixture(),
    image = f.bindings.get(f.image, f.loaders.read('LoadImage')!);
  await image.initialize();
  const initial = f.owner.diagnostics(),
    changed = jest.fn();
  f.owner.image(f.mask, 'image').subscribe(changed);
  image.changeSource({ kind: 'file', file: null });
  f.mask.widgets![1]!.value = 'blue';
  f.graph.afterChange?.();
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  document.body.append(popup);
  popup.append(document.createElement('input'));
  await settled();
  expect(f.owner.diagnostics()).toEqual(initial);
  expect(changed).not.toHaveBeenCalled();
  popup.remove();
  f.dispose();
});

test('releases old workflow identities and ignores queued callbacks after a new graph is activated', async () => {
  const f = fixture(),
    oldKey = f.owner.key(f.mask, 'image');
  f.disconnect(1);
  f.owner.beforeConfigureGraph();
  const next = associationGraph(),
    nodes = associationNodes();
  next.add(nodes.image, nodes.mask, nodes.consumer);
  next.connect(1, nodes.image, 0, nodes.consumer, 0);
  next.connect(2, nodes.mask, 0, nodes.consumer, 1);
  f.root(next.graph);
  f.owner.start();
  await settled();
  expect(f.owner.relationships.read(oldKey).kind).toBe('missing');
  expect(f.owner.relationships.read(f.owner.key(nodes.mask, 'image'))).toMatchObject({
    kind: 'associated',
    image: f.owner.key(nodes.image, 'image'),
  });
  f.dispose();
});

test('reconciles undo configuration and restores native callbacks when disposed', async () => {
  const f = fixture(),
    previousKey = f.owner.key(f.mask, 'image'),
    nodes = associationNodes();
  f.configure.mockImplementation(() => {
    f.graph._nodes = [];
    f.links.clear();
    f.add(nodes.image, nodes.mask, nodes.consumer);
    f.connect(1, nodes.image, 0, nodes.consumer, 0);
    f.connect(2, nodes.mask, 0, nodes.consumer, 1);
  });
  if (typeof f.graph.configure !== 'function') throw new Error('Missing native configure.');
  f.graph.configure({});
  await settled();
  expect(f.owner.relationships.read(previousKey).kind).toBe('missing');
  expect(f.owner.relationships.read(f.owner.key(nodes.mask, 'image')).kind).toBe('associated');
  f.dispose();
  expect(f.graph.configure).toBe(f.configure);
  expect(nodes.image.onConnectionsChange).toBeUndefined();
  expect(f.report).not.toHaveBeenCalled();
});

test('resolves an image exposed from a Cube output to an outside mask consumer', () => {
  const f = sourceMaskFixture();
  f.loaders.define({ LoadImage: {} });
  const outer = associationGraph(),
    inner = associationGraph(),
    nodes = associationNodes();
  inner.add(nodes.image);
  inner.links.set(50, {
    id: 50,
    origin_id: nodes.image.id!,
    origin_slot: 0,
    target_id: -20,
    target_slot: 0,
    type: 'IMAGE',
  });
  inner.graph.outputNode = { slots: [{ linkIds: [50] }] };
  const cube: ComfyNode = {
    id: 20,
    type: 'Cube',
    subgraph: inner.graph,
    isSubgraphNode: () => true,
    inputs: [],
    outputs: [{ name: 'pixels', type: 'IMAGE', links: [] }],
  };
  outer.add(cube, nodes.mask, nodes.consumer);
  outer.connect(1, cube, 0, nodes.consumer, 0);
  outer.connect(2, nodes.mask, 0, nodes.consumer, 1);
  const bindings = new ComfyInputBindings(f.loaders, f.ports),
    owner = new ComfyInputAssociations(f.loaders, bindings, () => outer.graph, f.report, {
      warn: jest.fn(),
    });
  owner.start();
  expect(owner.relationships.read(owner.key(nodes.mask, 'image'))).toMatchObject({
    kind: 'associated',
    image: owner.key(nodes.image, 'image'),
  });
  owner.dispose();
  bindings.clear();
  f.previews.dispose();
});

test('resolves an image exposed through a Cube input to the mask consumer inside that Cube', () => {
  const f = sourceMaskFixture();
  f.loaders.define({ LoadImage: {} });
  const outer = associationGraph(),
    inner = associationGraph(),
    nodes = associationNodes();
  inner.add(nodes.mask, nodes.consumer);
  inner.links.set(50, {
    id: 50,
    origin_id: -10,
    origin_slot: 0,
    target_id: nodes.consumer.id!,
    target_slot: 0,
    type: 'IMAGE',
  });
  nodes.consumer.inputs![0]!.link = 50;
  inner.graph.inputNode = { slots: [{ linkIds: [50] }] };
  inner.connect(2, nodes.mask, 0, nodes.consumer, 1);
  const cube: ComfyNode = {
    id: 20,
    type: 'Cube',
    subgraph: inner.graph,
    isSubgraphNode: () => true,
    inputs: [{ name: 'pixels', type: 'IMAGE', link: null }],
    outputs: [],
  };
  outer.add(nodes.image, cube);
  outer.connect(1, nodes.image, 0, cube, 0);
  const bindings = new ComfyInputBindings(f.loaders, f.ports),
    owner = new ComfyInputAssociations(f.loaders, bindings, () => outer.graph, f.report, {
      warn: jest.fn(),
    });
  owner.start();
  expect(owner.relationships.read(owner.key(nodes.mask, 'image'))).toMatchObject({
    kind: 'associated',
    image: owner.key(nodes.image, 'image'),
  });
  owner.dispose();
  bindings.clear();
  f.previews.dispose();
});

test('keeps equal node IDs in separate Cube instances independent and refreshes only affected scopes', async () => {
  const f = sourceMaskFixture();
  f.loaders.define({ LoadImage: {} });
  const root = associationGraph(),
    left = associationGraph(),
    right = associationGraph(),
    a = associationNodes(),
    b = associationNodes();
  for (const [native, nodes] of [
    [left, a],
    [right, b],
  ] as const) {
    native.add(nodes.image, nodes.mask, nodes.consumer);
    native.connect(1, nodes.image, 0, nodes.consumer, 0);
    native.connect(2, nodes.mask, 0, nodes.consumer, 1);
  }
  root.add(
    { id: 20, type: 'Cube', subgraph: left.graph, isSubgraphNode: () => true },
    { id: 30, type: 'Cube', subgraph: right.graph, isSubgraphNode: () => true },
  );
  const bindings = new ComfyInputBindings(f.loaders, f.ports),
    owner = new ComfyInputAssociations(f.loaders, bindings, () => root.graph, f.report, {
      warn: jest.fn(),
    });
  owner.start();
  expect(owner.key(a.mask, 'image')).not.toBe(owner.key(b.mask, 'image'));
  expect(owner.relationships.read(owner.key(b.mask, 'image'))).toMatchObject({
    kind: 'associated',
    image: owner.key(b.image, 'image'),
  });
  const initial = owner.diagnostics(),
    changed = jest.fn();
  owner.image(b.mask, 'image').subscribe(changed);
  left.disconnect(1);
  await settled();
  expect(owner.diagnostics().topologyReads - initial.topologyReads).toBe(2);
  expect(owner.diagnostics().membershipReads).toBe(initial.membershipReads);
  expect(changed).not.toHaveBeenCalled();
  owner.dispose();
  bindings.clear();
  f.previews.dispose();
});
