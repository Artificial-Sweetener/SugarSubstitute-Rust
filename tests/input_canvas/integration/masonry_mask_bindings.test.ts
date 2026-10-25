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
/** Prove mask source defaults, ordered identity, native undo and exact masonry presentation ownership. */
import { nativeInputFields, nativeSourceField } from '../support/native_input_fields.js';
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import { sourceMaskFixture } from '../support/source_mask_fixture.js';
import { maskPreviewAssociation } from '../support/association_graph.js';
import { ComfyInputAssociations } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputAssociations.js';
import { createComfyInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBinding.js';
import { ComfyInputBindings } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBindings.js';
import { MasonryInputEditor } from '../../../frontend/comfyui/ui/input_canvas/presentation/MasonryInputEditor.js';
import { MasonryInputHost } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/MasonryInputHost.js';
import { registerNativeNodeCard } from '../../../frontend/comfyui/ui/surface/NativeNodeCardBoundary.js';
import { isNativeWidgetPresented } from '../../../frontend/comfyui/ui/surface/NativeWidgetPresentation.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

beforeAll(() =>
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: randomUUID }),
);
beforeEach(() => {
  // jsdom supplies no native raster context; exercise pixel conversion against this external drawing boundary.
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    createImageData: (width: number, height: number) => ({
      data: new Uint8ClampedArray(width * height * 4),
    }),
    putImageData: jest.fn(),
  } as unknown as CanvasRenderingContext2D);
});
afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});
const loader = {
  kind: 'mask' as const,
  classType: 'SimpleSyrup.LoadMaskBatch',
  field: 'image',
  channel: 'channel',
  ordered: true,
};

/** Supply one exact graph node while native pixel and upload operations stay at the fixture boundary. */
async function fixture() {
  const f = sourceMaskFixture();
  const node: ComfyNode = {
    id: 4,
    type: loader.classType,
    widgets: [
      { name: 'image', value: [] },
      { name: 'channel', value: 'red' },
      { name: 'upload', type: 'button' },
    ],
    properties: {},
    graph: { beforeChange: jest.fn(), afterChange: jest.fn() },
  };
  const binding = createComfyInputBinding(node, loader, f.ports);
  await binding.initialize();
  return { ...f, node, binding };
}

/** Wait for committed metadata and preview microtasks without a native event-loop dependency. */
/** Invoke the exact dynamic Comfy callback after the host commits a widget value. */
function commitWidget(node: ComfyNode, name: string): unknown {
  const changed = node.onWidgetChanged;
  return typeof changed === 'function' ? changed.call(node, name) : undefined;
}

async function settled(): Promise<void> {
  for (let n = 0; n < 50; n++) await Promise.resolve();
}

test('defaults to live canvas selection, retains explicit sources during reorder and honors native metadata undo', async () => {
  const f = await fixture();
  expect(f.binding.state.entries[0]?.source).toEqual({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'selection' },
  });
  const first = f.binding.state.selected;
  f.binding.add();
  const second = f.binding.state.selected;
  f.binding.changeSource({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: 'doc',
    target: { kind: 'layer', layerId: 'b' },
  });
  const original = f.binding.state;
  f.binding.move(-1);
  expect(f.binding.state.entries.map((entry) => entry.id)).toEqual([second, first]);
  expect(f.binding.state.entries[0]?.source).toMatchObject({
    target: { kind: 'layer', layerId: 'b' },
  });
  f.node.properties = { sugarcubes_canvas_masks: original };
  expect(f.binding.state.entries.map((entry) => entry.id)).toEqual([first, second]);
  expect(f.node.graph?.beforeChange).toHaveBeenCalledTimes(4);
  expect(f.node.graph?.afterChange).toHaveBeenCalledTimes(4);
  f.binding.dispose();
  f.previews.dispose();
});

test('resolves canvas-backed inputs before native media validation without retaining an unrelated filename', async () => {
  const f = sourceMaskFixture(),
    node: ComfyNode = {
      id: 11,
      type: 'LoadImageMask',
      properties: {},
      widgets: [
        { name: 'image', value: 'unrelated-photo.png', options: { values: [] } },
        { name: 'channel', value: 'alpha' },
      ],
      graph: { beforeChange: jest.fn(), afterChange: jest.fn() },
    };
  const bindings = new ComfyInputBindings(f.loaders, f.ports);
  bindings.loaded(node);
  await bindings.ready();
  expect(node.widgets?.[0]?.value).toBe('');
  const binding = bindings.get(node, {
    kind: 'mask',
    classType: 'LoadImageMask',
    field: 'image',
    channel: 'channel',
    ordered: false,
  });
  expect(binding.state.files).toEqual([]);
  expect(binding.state.entries[0]?.source).toMatchObject({ target: { kind: 'selection' } });
  bindings.clear();
  f.previews.dispose();
});

test('publishes only confirmed real file choices and preserves ordinary files when no source-capable canvas is attached', async () => {
  const f = sourceMaskFixture(),
    node: ComfyNode = {
      id: 12,
      type: 'LoadImageMask',
      properties: {},
      widgets: [
        { name: 'image', value: 'original.png', options: { values: [] } },
        { name: 'channel', value: 'red' },
      ],
      graph: { beforeChange: jest.fn(), afterChange: jest.fn() },
    };
  const scalar = {
    kind: 'mask' as const,
    classType: 'LoadImageMask',
    field: 'image',
    channel: 'channel',
    ordered: false,
  };
  const binding = createComfyInputBinding(node, scalar, { ...f.ports, host: () => null });
  await binding.initialize();
  expect(node.widgets?.[0]?.value).toBe('original.png');
  expect(node.graph?.beforeChange).not.toHaveBeenCalled();
  binding.changeSource({ kind: 'file', file: 'verified-upload.png' });
  expect(node.widgets?.[0]?.value).toBe('verified-upload.png');
  expect(node.widgets?.[0]?.options?.values).toEqual(['verified-upload.png']);
  expect(binding.state.files).toEqual(['verified-upload.png']);
  binding.dispose();
  f.previews.dispose();
});

test('keeps graph file editing authoritative after views retire and preserves SimpleSyrup append intent', async () => {
  const f = await fixture();
  f.binding.dispose();
  const previous = jest.fn(() => 17);
  f.node.onWidgetChanged = previous;
  const add = { name: 'simple_syrup_add_media', callback: jest.fn() },
    replace = { name: 'simple_syrup_replace_media', callback: jest.fn() };
  f.node.widgets?.push(add, replace);
  const bindings = new ComfyInputBindings(f.loaders, f.ports),
    binding = bindings.get(f.node, loader);
  await binding.initialize();
  const original = binding.state.selected;
  add.callback();
  f.node.widgets![0]!.value = ['added.png'];
  expect(commitWidget(f.node, 'image')).toBe(17);
  expect(binding.state.entries.map((entry) => entry.id)).toContain(original);
  expect(binding.state.entries[1]?.source).toEqual({ kind: 'file', file: 'added.png' });
  replace.callback();
  f.node.widgets![0]!.value = ['replacement.png'];
  commitWidget(f.node, 'image');
  expect(binding.state.entries.map((entry) => entry.source)).toEqual([
    { kind: 'file', file: 'replacement.png' },
  ]);
  bindings.clear();
  expect(f.node.onWidgetChanged).toBe(previous);
  f.previews.dispose();
  expect(f.report).not.toHaveBeenCalled();
});

test('keeps row identity, renders literal labels and updates the effective mask when the channel changes', async () => {
  const f = await fixture(),
    paint = jest.fn();
  // Canvas is the external raster boundary in jsdom; retain the actual pixel conversion and source owners.
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    createImageData: (width: number, height: number) => ({
      data: new Uint8ClampedArray(width * height * 4),
    }),
    putImageData: paint,
  } as unknown as CanvasRenderingContext2D);
  f.binding.changeSource({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'current-layer' },
  });
  const state = f.binding.state;
  f.node.properties = {
    sugarcubes_canvas_masks: {
      ...state,
      entries: state.entries.map((entry) => ({ ...entry, label: '<img src=x onerror=boom>' })),
    },
  };
  const association = await maskPreviewAssociation(f, f.node, loader);
  const view = new MasonryInputEditor(
    document,
    f.binding,
    f.previews,
    async () => 'chosen.png',
    f.report,
    nativeInputFields(f.node),
    document.createElement('article'),
    association.source,
  );
  document.body.append(view.element);
  await settled();
  const row = view.element.querySelector('[data-input-entry]');
  expect(view.element.querySelector('img')).toBeNull();
  expect(view.element.textContent).toContain('<img src=x onerror=boom>');
  expect(paint).toHaveBeenCalled();
  const red = paint.mock.calls.at(-1)?.[0] as { data: Uint8ClampedArray };
  expect(red.data[0]).toBe(10);
  const captures = f.execute.mock.calls.filter(([query]) => query.type === 'capture').length;
  f.node.widgets![1]!.value = 'alpha';
  commitWidget(f.node, 'channel');
  await settled();
  const alpha = paint.mock.calls.at(-1)?.[0] as { data: Uint8ClampedArray };
  expect(alpha.data[0]).toBe(255);
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(captures);
  expect(view.element.querySelector('[data-input-entry]')).toBe(row);
  expect(nativeSourceField(f.node).value).toBe('current-layer');
  view.dispose();
  association.dispose();
  f.binding.dispose();
  f.previews.dispose();
  expect(f.report).not.toHaveBeenCalled();
});

test('mounts registered masonry replacements, preserves channel controls and restores presentation on collapse', async () => {
  const f = await fixture();
  f.binding.dispose();
  const bindings = new ComfyInputBindings(f.loaders, f.ports);
  nativeInputFields(f.node);
  const associations = new ComfyInputAssociations(
    f.loaders,
    bindings,
    () => f.node.graph,
    f.report,
    { warn: jest.fn() },
  );
  const host = new MasonryInputHost(
    document,
    f.loaders,
    f.previews,
    async () => 'chosen.png',
    f.report,
    bindings,
    associations,
  );
  const card = document.createElement('article');
  card.className = 'sugarcubes-workflow-masonry__card sugarcubes-cube-face__node-card';
  const release = registerNativeNodeCard(card, f.node);
  document.body.append(card);
  host.setup();
  await settled();
  expect(card.querySelector('[data-sc-input-editor]')).not.toBeNull();
  expect(isNativeWidgetPresented(f.node.widgets![0]!)).toBe(false);
  expect(isNativeWidgetPresented(f.node.widgets![1]!)).toBe(true);
  expect(card.getAttribute('data-sc-native-media')).toBe('replaced');
  const editor = card.querySelector('[data-sc-input-editor]'),
    calls = f.execute.mock.calls.length;
  const popup = document.createElement('div');
  document.body.append(popup);
  popup.append(document.createElement('input'));
  await settled();
  expect(card.querySelector('[data-sc-input-editor]')).toBe(editor);
  expect(f.execute).toHaveBeenCalledTimes(calls);
  f.node.flags = { collapsed: true };
  card.append(document.createElement('span'));
  await settled();
  expect(card.querySelector('[data-sc-input-editor]')).toBeNull();
  expect(isNativeWidgetPresented(f.node.widgets![0]!)).toBe(true);
  expect(card.hasAttribute('data-sc-native-media')).toBe(false);
  host.dispose();
  associations.dispose();
  bindings.clear();
  release();
});

test('routes custom loader interpretation through its actual settings and refreshes non-channel edits', async () => {
  const f = sourceMaskFixture();
  const custom = {
    kind: 'mask' as const,
    classType: 'CustomMask',
    field: 'file',
    ordered: false,
    channel: null,
  };
  const node: ComfyNode = {
    id: 99,
    type: custom.classType,
    properties: {},
    widgets: [
      { name: 'file', value: '' },
      { name: 'threshold', value: 0.4 },
      { name: 'invert', value: false },
    ],
  };
  const binding = createComfyInputBinding(node, custom, f.ports);
  await binding.initialize();
  const association = await maskPreviewAssociation(f, node, custom);
  const view = new MasonryInputEditor(
    document,
    binding,
    f.previews,
    async () => 'file.png',
    f.report,
    nativeInputFields(node),
    document.createElement('article'),
    association.source,
  );
  document.body.append(view.element);
  await settled();
  expect(f.files.interpret).toHaveBeenLastCalledWith(
    expect.anything(),
    {
      kind: 'mask',
      classType: 'CustomMask',
      field: 'file',
      inputs: { threshold: 0.4, invert: false },
    },
    256,
  );
  const captures = f.execute.mock.calls.filter(([query]) => query.type === 'capture').length;
  node.widgets![2]!.value = true;
  commitWidget(node, 'invert');
  await settled();
  expect(f.files.interpret).toHaveBeenLastCalledWith(
    expect.anything(),
    {
      kind: 'mask',
      classType: 'CustomMask',
      field: 'file',
      inputs: { threshold: 0.4, invert: true },
    },
    256,
  );
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(captures);
  expect(f.report).not.toHaveBeenCalled();
  view.dispose();
  association.dispose();
  binding.dispose();
  f.previews.dispose();
});

test('reconciles verified native file choices on restore even when authored source state is unchanged', async () => {
  const f = await fixture();
  f.binding.changeSource({ kind: 'file', file: 'sources/actual.png' });
  f.binding.dispose();
  const choices: string[] = [];
  f.node.widgets![0]!.options = { values: choices };
  const restored = createComfyInputBinding(f.node, loader, f.ports);
  await restored.initialize();
  expect(choices).toEqual(['sources/actual.png']);
  expect(restored.state.entries[0]?.source).toEqual({ kind: 'file', file: 'sources/actual.png' });
  expect(f.report).not.toHaveBeenCalled();
  restored.dispose();
  f.previews.dispose();
});

test('does not invent an effective preview channel when the loader setting comes from another node', async () => {
  const f = await fixture();
  f.node.inputs = [{ name: 'channel', link: 8, widget: { name: 'channel' } }];
  await expect(
    f.binding.preview.canvas({ width: 1, height: 1, format: 'rgba8', data: 'AAAAAA==' }, 256),
  ).rejects.toThrow('authored setting');
  f.binding.dispose();
  f.previews.dispose();
});
