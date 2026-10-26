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
/** Prove effective masks overlay the graph-associated loader's actual image through shared source owners. */
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import {
  sourceMaskFixture,
  sourceDocument,
  sourceCoverage,
} from '../support/source_mask_fixture.js';
import { nativeInputFields } from '../support/native_input_fields.js';
import { maskPreviewAssociation } from '../support/association_graph.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import { createComfyInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBinding.js';
import { MasonryInputEditor } from '../../../frontend/comfyui/ui/input_canvas/presentation/MasonryInputEditor.js';
import { publishPluginMode } from '../../../frontend/comfyui/ui/core/PluginMode.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import type { CanvasPixelRaster } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { nativePresentationWidgets } from '../../../frontend/comfyui/ui/surface/NativePresentationWidgets.js';

beforeAll(() =>
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: randomUUID }),
);
afterEach(() => {
  publishPluginMode(document, false);
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Settle shared source batching and interpretation without native UI or timer-driven capture. */
async function settled(): Promise<void> {
  for (let index = 0; index < 150; index++) await Promise.resolve();
}

/** Keep canvas capture and field interpretation real, replacing only native pixels and the DOM raster boundary. */
async function fixture(custom = false) {
  const f = sourceMaskFixture(),
    original = f.execute.getMockImplementation();
  if (!original) throw new Error('Missing native boundary.');
  const composition = encodeCanvasRaster(
    3,
    2,
    'rgba8',
    Uint8Array.from(Array.from({ length: 6 }, () => [80, 120, 160, 255]).flat()),
  );
  const layer = encodeCanvasRaster(
    3,
    2,
    'rgba8',
    Uint8Array.from(Array.from({ length: 6 }, () => [0, 255, 128, 64]).flat()),
  );
  let splitRevision = false;
  f.execute.mockImplementation(async (query) => {
    const value = await original(query);
    if (value.kind !== 'captured' || query.type !== 'capture') return value;
    return {
      ...value,
      snapshots: value.snapshots.map((snapshot) => ({
        ...snapshot,
        revision: splitRevision && query.maximumSize === 0 ? 'old' : 'current',
        pixels:
          snapshot.target.kind === 'composition'
            ? composition
            : snapshot.target.kind === 'selection'
              ? sourceCoverage
              : layer,
      })),
    };
  });
  const painted: Uint8ClampedArray[] = [];
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    createImageData: (width: number, height: number) => ({
      data: new Uint8ClampedArray(width * height * 4),
    }),
    putImageData: (image: { data: Uint8ClampedArray }) => painted.push(image.data.slice()),
  } as unknown as CanvasRenderingContext2D);
  publishPluginMode(document, true);
  const node: ComfyNode = {
    id: 4,
    type: 'LoadImageMask',
    properties: {},
    widgets: [
      { name: 'image', value: '' },
      { name: 'channel', value: 'red' },
    ],
  };
  const binding = createComfyInputBinding(
    node,
    {
      kind: 'mask',
      classType: custom ? 'CustomMask' : 'LoadImageMask',
      field: 'image',
      channel: 'channel',
      ordered: false,
    },
    f.ports,
  );
  await binding.initialize();
  const association = await maskPreviewAssociation(f, node, binding.loader);
  let view: MasonryInputEditor | null = null;
  return {
    ...f,
    node,
    binding,
    painted,
    association,
    mount: async () => {
      view = new MasonryInputEditor(
        document,
        binding,
        f.previews,
        async () => 'mask.png',
        f.report,
        nativeInputFields(node),
        document.createElement('article'),
        association.source,
      );
      document.body.append(view.element);
      await settled();
      return view;
    },
    channel: async (channel: string) => {
      node.widgets![1]!.value = channel;
      if (typeof node.onWidgetChanged === 'function') node.onWidgetChanged.call(node, 'channel');
      await settled();
    },
    split: (value: boolean) => {
      splitRevision = value;
    },
    changed: async () => {
      for (const listener of [...f.changes])
        listener({ documentId: sourceDocument.id, revision: 'current' });
      await settled();
    },
    dispose: () => {
      view?.dispose();
      association.dispose();
      binding.dispose();
      f.previews.dispose();
    },
  };
}

test('shows one wash over the associated composition source and reinterprets channels without recapturing it', async () => {
  const f = await fixture(),
    view = await f.mount();
  const frame = view.element.querySelector('.sc-input-editor__frame'),
    canvas = view.element.querySelector('canvas');
  expect([...f.painted.at(-1)!.slice(0, 8)]).toEqual([40, 60, 80, 255, 168, 188, 208, 255]);
  const captures = f.execute.mock.calls.filter(([query]) => query.type === 'capture');
  expect(captures).toHaveLength(1);
  expect(captures[0]?.[0]).toMatchObject({
    targets: expect.arrayContaining([{ kind: 'composition' }, { kind: 'selection' }]),
    maximumSize: 256,
  });
  f.binding.changeSource({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'current-layer' },
  });
  await settled();
  const count = f.execute.mock.calls.filter(([query]) => query.type === 'capture').length;
  for (const [channel, expected] of [
    ['red', [40, 60, 80, 255]],
    ['green', [168, 188, 208, 255]],
    ['blue', [104, 124, 144, 255]],
    ['alpha', [136, 156, 176, 255]],
  ] as const) {
    await f.channel(channel);
    expect([...f.painted.at(-1)!.slice(0, 4)]).toEqual(expected);
  }
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(count);
  expect(view.element.querySelector('.sc-input-editor__frame')).toBe(frame);
  expect(view.element.querySelector('canvas')).toBe(canvas);
  expect(f.stage).not.toHaveBeenCalled();
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('native mask fields default to no crop and switch between persistent context and actual-input previews', async () => {
  const f = await fixture(),
    read = f.execute.getMockImplementation();
  if (!read) throw new Error('Missing canvas reader');
  const selected = encodeCanvasRaster(3, 2, 'coverage8', Uint8Array.from([0, 255, 0, 0, 0, 0]));
  f.execute.mockImplementation(async (query) => {
    const result = await read(query);
    return result.kind === 'captured'
      ? {
          ...result,
          snapshots: result.snapshots.map((snapshot) =>
            snapshot.target.kind === 'selection' ? { ...snapshot, pixels: selected } : snapshot,
          ),
        }
      : result;
  });
  const view = await f.mount(),
    canvas = view.element.querySelector('canvas'),
    frame = view.element.querySelector<HTMLElement>('.sc-input-editor__frame');
  const crop = nativePresentationWidgets(f.node).find((widget) => widget.label === 'Crop Factor');
  expect(crop).toMatchObject({ type: 'number', value: 0, serialize: false });
  for (const label of ['Mask Dilation', 'Crop Factor', 'Post Dilation', 'Feather'])
    expect(
      nativePresentationWidgets(f.node).find((widget) => widget.label === label),
    ).toMatchObject({ type: 'number', value: 0, options: { presentation: 'spinner-slider' } });
  crop?.callback?.(1);
  expect(frame?.style.aspectRatio).toBe('3 / 2');
  await settled();
  const outline = view.element.querySelector<HTMLElement>('.sc-input-editor__crop');
  expect(outline?.hidden).toBe(false);
  expect(outline?.style.left).toBe('33.333333333333336%');
  expect(canvas?.width).toBe(3);
  const captures = f.execute.mock.calls.filter(([query]) => query.type === 'capture').length;
  const paints = f.painted.length;
  const mode = nativePresentationWidgets(f.node).find((widget) => widget.label === 'Preview');
  mode?.callback?.('input');
  expect(frame?.style.aspectRatio).toBe('1 / 1');
  await settled();
  expect(outline?.hidden).toBe(true);
  expect(canvas?.width).toBe(3);
  expect(canvas?.height).toBe(2);
  expect(canvas?.style.width).toBe('300%');
  expect(canvas?.style.left).toBe('-100%');
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(captures);
  expect(f.painted).toHaveLength(paints);
  expect(frame?.style.aspectRatio).toBe('1 / 1');
  expect(view.element.querySelector('canvas')).toBe(canvas);
  expect(view.element.querySelector('.sc-input-editor__frame')).toBe(frame);
  expect(f.binding.state.edits?.crop_factor).toBe(1);
  mode?.callback?.('context');
  expect(canvas?.style.width).toBe('100%');
  expect(canvas?.style.left).toBe('0%');
  expect(f.painted).toHaveLength(paints);
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('reuses interpreted file masks while composition updates repaint the persistent preview', async () => {
  const f = await fixture(),
    view = await f.mount();
  const canvas = view.element.querySelector('canvas');
  f.binding.changeSource({ kind: 'file', file: 'mask.png' });
  await settled();
  expect([...f.painted.at(-1)!.slice(0, 4)]).toEqual([40, 60, 80, 255]);
  const decodes = f.files.load.mock.calls.length,
    paints = f.painted.length;
  await f.changed();
  expect(f.painted.length).toBeGreaterThan(paints);
  expect(f.files.load).toHaveBeenCalledTimes(decodes);
  expect(view.element.querySelector('canvas')).toBe(canvas);
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('keeps custom-loader interpretation separate from a bounded composition and waits for matching revisions', async () => {
  const f = await fixture(true);
  f.split(true);
  const view = await f.mount();
  expect(f.painted).toHaveLength(0);
  expect(view.element.querySelector('canvas')?.hidden).toBe(true);
  expect(
    f.execute.mock.calls.flatMap(([query]) =>
      query.type === 'capture' ? [query.maximumSize] : [],
    ),
  ).toEqual(expect.arrayContaining([256, 0]));
  f.split(false);
  await f.changed();
  expect(f.painted.length).toBeGreaterThan(0);
  expect([...f.painted.at(-1)!.slice(0, 4)]).toEqual([40, 60, 80, 255]);
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('follows the associated image loader source without recapturing the mask or rebuilding graph relationships', async () => {
  const f = await fixture(),
    view = await f.mount();
  const frame = view.element.querySelector('.sc-input-editor__frame'),
    canvas = view.element.querySelector('canvas'),
    initial = f.association.associations.diagnostics();
  f.association.image.changeSource({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'current-layer' },
  });
  await settled();
  expect([...f.painted.at(-1)!.slice(0, 8)]).toEqual([0, 51, 26, 160, 204, 255, 230, 160]);
  const captures = f.execute.mock.calls.filter(([query]) => query.type === 'capture');
  expect(captures).toHaveLength(2);
  expect(captures.at(-1)?.[0]).toMatchObject({ targets: [{ kind: 'current-layer' }] });
  expect(f.association.associations.diagnostics()).toEqual(initial);
  expect(view.element.querySelector('.sc-input-editor__frame')).toBe(frame);
  expect(view.element.querySelector('canvas')).toBe(canvas);
  f.dispose();
});

test('keeps the rendered image while its replacement loads, then adopts the loaded file aspect ratio', async () => {
  const f = await fixture(),
    view = await f.mount();
  const frame = view.element.querySelector<HTMLElement>('.sc-input-editor__frame'),
    canvas = view.element.querySelector('canvas'),
    previous = f.painted.at(-1);
  let finish: ((pixels: CanvasPixelRaster) => void) | undefined;
  f.files.load.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  f.association.image.changeSource({ kind: 'file', file: 'associated-image.png' });
  await settled();
  expect(canvas?.hidden).toBe(false);
  expect(frame?.style.aspectRatio).toBe('3 / 2');
  expect(f.painted.at(-1)).toBe(previous);
  finish?.(
    encodeCanvasRaster(
      2,
      2,
      'rgba8',
      Uint8Array.from(Array.from({ length: 4 }, () => [200, 40, 100, 255]).flat()),
    ),
  );
  await settled();
  expect([...f.painted.at(-1)!.slice(0, 4)]).toEqual([132, 52, 82, 255]);
  expect(frame?.style.aspectRatio).toBe('2 / 2');
  expect(view.element.querySelector('canvas')).toBe(canvas);
  expect(view.element.querySelector('.sc-input-editor__frame')).toBe(frame);
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('uses solid black for absent, empty and unavailable associated images and recovers without a composition fallback', async () => {
  const f = await fixture();
  f.association.disconnect(1);
  await settled();
  const view = await f.mount(),
    frame = view.element.querySelector<HTMLElement>('.sc-input-editor__frame');
  expect(frame?.dataset.content).toBe('black');
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(0);
  f.association.connect(3, f.association.imageNode, 0, f.association.consumer, 0);
  await settled();
  expect(frame?.dataset.content).toBe('pixels');
  f.association.image.changeSource({ kind: 'file', file: null });
  await settled();
  const count = f.execute.mock.calls.filter(([query]) => query.type === 'capture').length;
  expect(frame?.dataset.content).toBe('black');
  expect(frame?.querySelector('canvas')?.hidden).toBe(true);
  const execute = f.execute.getMockImplementation();
  if (!execute) throw new Error('Missing source boundary.');
  let selected = false;
  f.execute.mockImplementation(async (query) => {
    const value = await execute(query);
    return value.kind === 'context' ? { ...value, currentLayerId: selected ? 'a' : null } : value;
  });
  await f.changed();
  f.association.image.changeSource({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'current-layer' },
  });
  await settled();
  expect(frame?.dataset.content).toBe('black');
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(count);
  f.files.load.mockRejectedValueOnce(new Error('File cannot be decoded'));
  f.association.image.changeSource({ kind: 'file', file: 'broken.png' });
  await settled();
  expect(frame?.dataset.content).toBe('black');
  expect(f.report).toHaveBeenCalledWith(
    expect.objectContaining({ message: 'File cannot be decoded' }),
  );
  selected = true;
  await f.changed();
  f.association.image.changeSource({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'current-layer' },
  });
  await settled();
  expect(frame?.dataset.content).toBe('pixels');
  expect([...f.painted.at(-1)!.slice(0, 4)]).toEqual([0, 51, 26, 160]);
  f.dispose();
});

test('clears to black for a competing image connection and restores the actual image after disconnect', async () => {
  const f = await fixture(),
    view = await f.mount();
  f.association.consumer.inputs!.push({ name: 'secondImage', type: 'IMAGE', link: null });
  f.association.connect(3, f.association.imageNode, 0, f.association.consumer, 2);
  await settled();
  expect(view.element.querySelector<HTMLElement>('.sc-input-editor__frame')?.dataset.content).toBe(
    'black',
  );
  f.association.disconnect(3);
  await settled();
  expect(view.element.querySelector<HTMLElement>('.sc-input-editor__frame')?.dataset.content).toBe(
    'pixels',
  );
  expect([...f.painted.at(-1)!.slice(0, 4)]).toEqual([40, 60, 80, 255]);
  f.dispose();
});
