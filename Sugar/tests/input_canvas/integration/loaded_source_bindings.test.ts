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
/** Prove automatic loading respects native source links without suppressing explicit or genuine failures. */
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import { sourceMaskFixture } from '../support/source_mask_fixture.js';
import { associationGraph } from '../support/association_graph.js';
import { ComfyInputBindings } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBindings.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import type { InputLoader } from '../../../frontend/comfyui/ui/input_canvas/domain/InputLoader.js';
import type { CanvasSourceResult } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { withInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/InputBindingMetadata.js';

beforeAll(() =>
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: randomUUID }),
);
const loader: InputLoader = {
  kind: 'image',
  classType: 'LoadImage',
  field: 'image',
  channel: null,
  ordered: false,
};

/** Hold an external response until the native topology transaction has completed. */
function deferred<T>() {
  const releases: ((value: T) => void)[] = [];
  const promise = new Promise<T>((resolve) => releases.push(resolve));
  return { promise, release: (value: T): void => releases[0]!(value) };
}

/** Supply the actual native input-node link representation produced by subgraph conversion. */
function nestedLoader() {
  const inner = associationGraph();
  const node: ComfyNode = {
    id: 1,
    type: 'LoadImage',
    properties: {},
    widgets: [{ name: 'image', value: 'authored.png' }],
    inputs: [{ name: 'image', type: 'COMBO', link: 0, widget: { name: 'image' } }],
    outputs: [{ name: 'IMAGE', type: 'IMAGE', links: [] }],
  };
  inner.add(node);
  inner.links.set(0, {
    id: 0,
    origin_id: -10,
    origin_slot: 0,
    target_id: 1,
    target_slot: 0,
    type: 'COMBO',
  });
  inner.graph.inputNode = { slots: [{ linkIds: [0] }] };
  const cube: ComfyNode = {
    id: 3,
    type: 'Cube',
    subgraph: inner.graph,
    isSubgraphNode: () => true,
    inputs: [{ name: 'image', type: 'COMBO', link: null }],
    outputs: [],
  };
  return { inner, node, cube };
}

test('does not initialize a linked nested source during automatic loading, while unlinked siblings initialize', async () => {
  const f = sourceMaskFixture();
  f.loaders.define({ LoadImage: {} });
  const { inner, node, cube } = nestedLoader();
  const sibling: ComfyNode = {
    id: 2,
    type: 'LoadImage',
    properties: {},
    widgets: [{ name: 'image', value: '' }],
    inputs: [{ name: 'image', link: null }],
  };
  inner.add(sibling);
  const bindings = new ComfyInputBindings(f.loaders, f.ports);
  try {
    bindings.loaded(cube);
    await bindings.ready();
    expect(f.report).not.toHaveBeenCalled();
    expect(node.widgets?.[0]?.value).toBe('authored.png');
    expect(node.properties).toEqual({});
    expect(node.onWidgetChanged).toBeUndefined();
    expect(node.onRemoved).toBeUndefined();
    expect(bindings.get(sibling, loader).state.entries[0]?.source.kind).toBe('canvas');
  } finally {
    bindings.clear();
    f.previews.dispose();
  }
  expect(f.changes.size).toBe(0);
});

test('still refuses explicit canvas source access to a linked nested field', async () => {
  const f = sourceMaskFixture();
  const { node } = nestedLoader();
  const bindings = new ComfyInputBindings(f.loaders, f.ports);
  try {
    const binding = bindings.get(node, loader);
    await binding.initialize();
    expect(binding.error).toBe(
      'Disconnect the linked source field before selecting a canvas source.',
    );
    expect(f.report).toHaveBeenCalledWith(new Error(binding.error!));
    expect(node.widgets?.[0]?.value).toBe('authored.png');
    expect(node.properties).toEqual({});
  } finally {
    bindings.clear();
    f.previews.dispose();
  }
  expect(f.changes.size).toBe(0);
});

test('automatic graph and card lookups do not initialize a newly promoted linked source', async () => {
  const f = sourceMaskFixture();
  const { node } = nestedLoader();
  const bindings = new ComfyInputBindings(f.loaders, f.ports);
  try {
    const binding = bindings.get(node, loader);
    await bindings.ready();
    expect(f.report).not.toHaveBeenCalled();
    expect(binding.error).toBeNull();
    expect(node.widgets?.[0]?.value).toBe('authored.png');
    expect(node.properties).toEqual({});
    node.inputs![0]!.link = null;
    await bindings.ready();
    expect(binding.state.entries[0]?.source.kind).toBe('canvas');
    expect(f.report).not.toHaveBeenCalled();
  } finally {
    bindings.clear();
    f.previews.dispose();
  }
  expect(f.changes.size).toBe(0);
});

test('automatic readiness rechecks a retained source after Cube promotion links its native input', async () => {
  const f = sourceMaskFixture();
  const { inner, node, cube } = nestedLoader();
  const root = associationGraph();
  inner.graph.remove?.(node);
  node.inputs![0]!.link = null;
  root.add(node);
  const bindings = new ComfyInputBindings(f.loaders, f.ports);
  try {
    const binding = bindings.get(node, loader);
    await bindings.ready();
    const properties = JSON.stringify(node.properties),
      file = node.widgets?.[0]?.value;
    expect(f.report).not.toHaveBeenCalled();
    root.graph.remove?.(node);
    inner.add(node);
    node.inputs![0]!.link = 0;
    root.add(cube);
    bindings.loaded(cube);
    expect(bindings.get(node, loader)).toBe(binding);
    await bindings.ready();
    await bindings.ready();
    expect(f.report).not.toHaveBeenCalled();
    expect(binding.error).toBeNull();
    expect(JSON.stringify(node.properties)).toBe(properties);
    expect(node.widgets?.[0]?.value).toBe(file);
    await binding.initialize();
    expect(binding.error).toBe(
      'Disconnect the linked source field before selecting a canvas source.',
    );
    expect(f.report).toHaveBeenCalledTimes(1);
    node.inputs![0]!.link = null;
    await bindings.ready();
    expect(binding.error).toBeNull();
    expect(f.report).toHaveBeenCalledTimes(1);
  } finally {
    bindings.clear();
    f.previews.dispose();
  }
  expect(f.changes.size).toBe(0);
});

test('queued automatic initialization rechecks promotion after awaiting the canvas context', async () => {
  const f = sourceMaskFixture();
  const { node } = nestedLoader();
  node.inputs![0]!.link = null;
  const context = await f.execute({ type: 'context' });
  const entered = deferred<void>(),
    response = deferred<CanvasSourceResult>();
  f.execute.mockImplementationOnce(() => {
    entered.release();
    return response.promise;
  });
  const bindings = new ComfyInputBindings(f.loaders, f.ports);
  try {
    bindings.get(node, loader);
    const pending = bindings.ready();
    await entered.promise;
    node.inputs![0]!.link = 0;
    response.release(context);
    await pending;
    expect(f.report).not.toHaveBeenCalled();
    expect(node.properties).toEqual({});
    expect(node.widgets?.[0]?.value).toBe('authored.png');
    node.inputs![0]!.link = null;
    await bindings.ready();
    expect(bindings.get(node, loader).state.entries[0]?.source.kind).toBe('canvas');
  } finally {
    bindings.clear();
    f.previews.dispose();
  }
  expect(f.changes.size).toBe(0);
});

test('automatic restoration does not publish a saved source after promotion during file validation', async () => {
  const f = sourceMaskFixture();
  const { node } = nestedLoader();
  node.inputs![0]!.link = null;
  node.widgets![0]!.value = 'saved.png';
  node.properties = withInputBinding({}, 'image', {
    version: 2,
    field: 'image',
    files: ['saved.png'],
    selected: 'saved',
    entries: [{ id: 'saved', label: 'Image 1', source: { kind: 'file', file: 'saved.png' } }],
  });
  const properties = node.properties;
  const entered = deferred<void>(),
    response = deferred<boolean>();
  const bindings = new ComfyInputBindings(f.loaders, {
    ...f.ports,
    verifyFile: () => {
      entered.release();
      return response.promise;
    },
  });
  try {
    bindings.get(node, loader);
    const pending = bindings.ready();
    await entered.promise;
    node.inputs![0]!.link = 0;
    node.widgets![0]!.value = 'promoted.png';
    response.release(true);
    await pending;
    expect(f.report).not.toHaveBeenCalled();
    expect(node.properties).toEqual(properties);
    expect(node.widgets?.[0]?.value).toBe('promoted.png');
  } finally {
    bindings.clear();
    f.previews.dispose();
  }
  expect(f.changes.size).toBe(0);
});

test('still reports genuine source restoration errors during automatic loading', async () => {
  const f = sourceMaskFixture();
  const error = new Error('Saved source could not be restored.');
  const bindings = new ComfyInputBindings(f.loaders, {
    ...f.ports,
    legacy: {
      read: async () => {
        throw error;
      },
    },
  });
  const node: ComfyNode = {
    id: 4,
    type: 'LoadImageMask',
    properties: {},
    widgets: [{ name: 'image', value: '' }],
  };
  try {
    bindings.loaded(node);
    await bindings.ready();
    expect(f.report).toHaveBeenCalledWith(error);
    expect(node.properties).toEqual({});
  } finally {
    bindings.clear();
    f.previews.dispose();
  }
  expect(f.changes.size).toBe(0);
});
